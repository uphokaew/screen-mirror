use anyhow::{Context as AnyhowContext, Result};
use bytes::Bytes;
use ffmpeg::codec::Context;
use ffmpeg::codec::decoder::Video as VideoDecoder;
use ffmpeg::codec::parameters::Parameters;
use ffmpeg::format::Pixel;
use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};
use ffmpeg::util::frame::video::Video as VideoFrame;
use ffmpeg_next as ffmpeg;
use std::collections::VecDeque;

/// Pixel format for decoded frames
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    YUV420P,
    NV12,
    RGBA,
}

impl PixelFormat {
    pub fn to_ffmpeg(&self) -> Pixel {
        match self {
            PixelFormat::YUV420P => Pixel::YUV420P,
            PixelFormat::NV12 => Pixel::NV12,
            PixelFormat::RGBA => Pixel::RGBA,
        }
    }

    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::YUV420P => 1, // Actually 1.5 bytes/pixel but we handle planes separately
            PixelFormat::NV12 => 1,
            PixelFormat::RGBA => 4,
        }
    }
}

/// Decoded video frame with metadata
pub struct DecodedFrame {
    pub pts: i64,
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
}

impl DecodedFrame {
    /// Get stride (bytes per row) for the frame
    pub fn stride(&self) -> usize {
        self.width as usize * self.format.bytes_per_pixel()
    }
}

/// Hardware-accelerated video decoder
pub struct HardwareVideoDecoder {
    decoder: VideoDecoder,
    scaler: Option<ScalingContext>,
    #[allow(dead_code)]
    frame_queue: VecDeque<DecodedFrame>,
    output_format: PixelFormat,
    packet_buffer: Vec<u8>,
}

impl HardwareVideoDecoder {
    /// Create a new hardware-accelerated video decoder
    ///
    /// # Arguments
    /// * `hw_decoder` - Hardware decoder preference: "auto", "nvdec", "qsv", "vaapi", "none"
    /// * `output_format` - Desired output pixel format
    pub fn new(hw_decoder: &str, output_format: PixelFormat) -> Result<Self> {
        // Initialize FFmpeg
        ffmpeg::init().context("Failed to initialize FFmpeg")?;

        // Find decoder based on hardware preference
        let decoder = Self::create_decoder(hw_decoder)?;

        Ok(Self {
            decoder,
            scaler: None,
            frame_queue: VecDeque::new(),
            output_format,
            packet_buffer: Vec::new(),
        })
    }

    /// Create hardware or software decoder based on preference
    fn create_decoder(hw_decoder: &str) -> Result<VideoDecoder> {
        match hw_decoder.to_lowercase().as_str() {
            "nvdec" => {
                // Try NVDEC (NVIDIA hardware decoding)
                Self::try_hw_decoder(&["h264_cuvid", "hevc_cuvid"])
                    .or_else(|_| Self::create_software_decoder())
            }
            "qsv" => {
                // Try QSV (Intel Quick Sync Video)
                Self::try_hw_decoder(&["h264_qsv", "hevc_qsv"])
                    .or_else(|_| Self::create_software_decoder())
            }
            "vaapi" => {
                // Try VAAPI (Video Acceleration API for Linux/AMD)
                Self::try_hw_decoder(&["h264_vaapi", "hevc_vaapi"])
                    .or_else(|_| Self::create_software_decoder())
            }
            "auto" => {
                // Try hardware decoders in order of preference
                Self::try_hw_decoder(&["h264_cuvid", "hevc_cuvid"])
                    .or_else(|_| Self::try_hw_decoder(&["h264_qsv", "hevc_qsv"]))
                    .or_else(|_| Self::try_hw_decoder(&["h264_vaapi", "hevc_vaapi"]))
                    .or_else(|_| Self::create_software_decoder())
            }
            _ => {
                // Use software decoder
                Self::create_software_decoder()
            }
        }
    }

    /// Create a context with the specified codec
    fn create_context(codec: &ffmpeg::Codec) -> Result<Context> {
        let mut params = Parameters::new();
        unsafe {
            (*params.as_mut_ptr()).codec_id = codec.id().into();
        }
        Ok(Context::from_parameters(params)?)
    }

    /// Try to create a hardware decoder
    fn try_hw_decoder(codec_names: &[&str]) -> Result<VideoDecoder> {
        for codec_name in codec_names {
            if let Some(codec) = ffmpeg::codec::decoder::find_by_name(codec_name) {
                let context = Self::create_context(&codec)?;
                if let Ok(decoder) = context.decoder().video() {
                    tracing::info!("Using hardware decoder: {}", codec_name);
                    return Ok(decoder);
                }
            }
        }
        Err(anyhow::anyhow!("No hardware decoder available"))
    }

    /// Create software decoder (fallback)
    fn create_software_decoder() -> Result<VideoDecoder> {
        // Try H.264 first, then H.265
        if let Some(codec) = ffmpeg::codec::decoder::find_by_name("h264") {
            let context = Self::create_context(&codec)?;
            if let Ok(decoder) = context.decoder().video() {
                tracing::info!("Using software H.264 decoder");
                return Ok(decoder);
            }
        }

        if let Some(codec) = ffmpeg::codec::decoder::find_by_name("hevc") {
            let context = Self::create_context(&codec)?;
            if let Ok(decoder) = context.decoder().video() {
                tracing::info!("Using software H.265 decoder");
                return Ok(decoder);
            }
        }

        Err(anyhow::anyhow!("No video decoder available"))
    }

    /// Decode a video packet
    ///
    /// # Arguments
    /// * `data` - Encoded video data (H.264/H.265 NALUs)
    /// * `pts` - Presentation timestamp in microseconds
    ///
    /// # Returns
    /// Decoded frame if a complete frame was produced, None otherwise
    pub fn decode(&mut self, data: &Bytes, pts: i64) -> Result<Option<DecodedFrame>> {
        // Append data to packet buffer
        self.packet_buffer.extend_from_slice(data);

        // Create packet from buffer
        let mut packet = ffmpeg::codec::packet::Packet::copy(&self.packet_buffer);
        packet.set_pts(Some(pts));

        // Send packet to decoder
        self.decoder
            .send_packet(&packet)
            .context("Failed to send packet to decoder")?;

        // Clear packet buffer after successful send
        self.packet_buffer.clear();

        // Try to receive decoded frame
        let mut frame = VideoFrame::empty();
        match self.decoder.receive_frame(&mut frame) {
            Ok(_) => {
                // Frame decoded successfully
                let decoded = self.convert_frame(&frame, pts)?;
                Ok(Some(decoded))
            }
            Err(ffmpeg::Error::Other { errno: 11 }) => {
                // EAGAIN - need more data
                Ok(None)
            }
            Err(e) => Err(anyhow::anyhow!("Decoder error: {:?}", e)),
        }
    }

    /// Convert FFmpeg frame to our DecodedFrame format
    fn convert_frame(&mut self, frame: &VideoFrame, pts: i64) -> Result<DecodedFrame> {
        let width = frame.width();
        let height = frame.height();
        let src_format = frame.format();
        let dst_format = self.output_format.to_ffmpeg();

        // Check if we need to scale/convert format
        let final_frame = if src_format != dst_format {
            // Initialize scaler if needed
            if self.scaler.is_none() {
                self.scaler = Some(
                    ScalingContext::get(
                        src_format,
                        width,
                        height,
                        dst_format,
                        width,
                        height,
                        Flags::BILINEAR,
                    )
                    .context("Failed to create scaling context")?,
                );
            }

            // Scale/convert frame
            let mut converted = VideoFrame::empty();
            self.scaler
                .as_mut()
                .unwrap()
                .run(frame, &mut converted)
                .context("Failed to scale frame")?;
            converted
        } else {
            frame.clone()
        };

        // Extract frame data to contiguous buffer
        let data = self.extract_frame_data(&final_frame)?;

        Ok(DecodedFrame {
            pts,
            data,
            width,
            height,
            format: self.output_format,
        })
    }

    /// Extract frame data to a contiguous Vec<u8>
    fn extract_frame_data(&self, frame: &VideoFrame) -> Result<Vec<u8>> {
        match self.output_format {
            PixelFormat::RGBA => {
                // RGBA is packed, single plane
                let stride = frame.stride(0);
                let width = frame.width() as usize;
                let height = frame.height() as usize;
                let data = frame.data(0);

                let mut buffer = Vec::with_capacity(width * height * 4);

                for y in 0..height {
                    let row_start = y * stride;
                    let row_end = row_start + (width * 4);
                    buffer.extend_from_slice(&data[row_start..row_end]);
                }

                Ok(buffer)
            }
            PixelFormat::YUV420P => {
                // YUV420P has 3 planes: Y, U, V
                let width = frame.width() as usize;
                let height = frame.height() as usize;

                let y_plane = frame.data(0);
                let u_plane = frame.data(1);
                let v_plane = frame.data(2);

                let y_stride = frame.stride(0);
                let u_stride = frame.stride(1);
                let v_stride = frame.stride(2);

                // Calculate buffer size (Y + U + V)
                let y_size = width * height;
                let uv_size = (width / 2) * (height / 2);
                let total_size = y_size + uv_size + uv_size;

                let mut buffer = Vec::with_capacity(total_size);

                // Copy Y plane
                for y in 0..height {
                    let row_start = y * y_stride;
                    let row_end = row_start + width;
                    buffer.extend_from_slice(&y_plane[row_start..row_end]);
                }

                // Copy U plane
                for y in 0..(height / 2) {
                    let row_start = y * u_stride;
                    let row_end = row_start + (width / 2);
                    buffer.extend_from_slice(&u_plane[row_start..row_end]);
                }

                // Copy V plane
                for y in 0..(height / 2) {
                    let row_start = y * v_stride;
                    let row_end = row_start + (width / 2);
                    buffer.extend_from_slice(&v_plane[row_start..row_end]);
                }

                Ok(buffer)
            }
            PixelFormat::NV12 => {
                // NV12 has 2 planes: Y, UV interleaved
                let width = frame.width() as usize;
                let height = frame.height() as usize;

                let y_plane = frame.data(0);
                let uv_plane = frame.data(1);

                let y_stride = frame.stride(0);
                let uv_stride = frame.stride(1);

                let y_size = width * height;
                let uv_size = width * (height / 2);
                let total_size = y_size + uv_size;

                let mut buffer = Vec::with_capacity(total_size);

                // Copy Y plane
                for y in 0..height {
                    let row_start = y * y_stride;
                    let row_end = row_start + width;
                    buffer.extend_from_slice(&y_plane[row_start..row_end]);
                }

                // Copy UV plane
                for y in 0..(height / 2) {
                    let row_start = y * uv_stride;
                    let row_end = row_start + width;
                    buffer.extend_from_slice(&uv_plane[row_start..row_end]);
                }

                Ok(buffer)
            }
        }
    }

    /// Flush the decoder and get any remaining frames
    pub fn flush(&mut self) -> Result<Vec<DecodedFrame>> {
        let mut frames = Vec::new();

        // Send flush signal
        self.decoder
            .send_eof()
            .context("Failed to send EOF to decoder")?;

        // Receive all remaining frames
        loop {
            let mut frame = VideoFrame::empty();
            match self.decoder.receive_frame(&mut frame) {
                Ok(_) => {
                    if let Ok(decoded) = self.convert_frame(&frame, 0) {
                        frames.push(decoded);
                    }
                }
                Err(_) => break,
            }
        }

        Ok(frames)
    }

    /// Get decoder information
    pub fn info(&self) -> String {
        format!(
            "Decoder: {}x{}, Format: {:?}",
            self.decoder.width(),
            self.decoder.height(),
            self.output_format
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_creation() {
        // Test that decoder can be created (may fall back to software)
        let result = HardwareVideoDecoder::new("auto", PixelFormat::RGBA);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pixel_format_conversion() {
        assert_eq!(PixelFormat::RGBA.bytes_per_pixel(), 4);
        assert_eq!(PixelFormat::YUV420P.bytes_per_pixel(), 1);
    }
}
