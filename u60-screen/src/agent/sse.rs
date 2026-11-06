use super::http::Error;

const MAX_FRAME: usize = 256 * 1024;
#[derive(Default)]
pub struct Decoder {
    pending: Vec<u8>,
    event: String,
    data: String,
}
pub struct Frame {
    pub event: String,
    pub data: String,
}
impl Decoder {
    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<Frame>, Error> {
        if self.pending.len() + chunk.len() > MAX_FRAME {
            return Err(Error::InvalidResponse);
        }
        self.pending.extend_from_slice(chunk);
        let mut frames = Vec::new();
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            let line = self.pending.drain(..=end).collect::<Vec<_>>();
            let line = std::str::from_utf8(&line)
                .map_err(|_| Error::InvalidResponse)?
                .trim_end_matches(['\r', '\n'])
                .trim_start_matches('\u{feff}');
            if line.is_empty() {
                if !self.data.is_empty() {
                    self.data.pop();
                    frames.push(Frame {
                        event: std::mem::take(&mut self.event),
                        data: std::mem::take(&mut self.data),
                    });
                }
                self.event.clear();
                continue;
            }
            let (field, value) = line.split_once(':').unwrap_or((line, ""));
            let value = value.strip_prefix(' ').unwrap_or(value);
            match field {
                "event" if value.len() <= 64 => self.event = value.to_string(),
                "data" => {
                    self.data.push_str(value);
                    self.data.push('\n');
                }
                _ => {}
            }
            if self.data.len() > MAX_FRAME {
                return Err(Error::InvalidResponse);
            }
        }
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_utf8_comments_and_multiline_events_are_decoded_and_bounded() {
        let wire = "retry: 3000\r\n: keepalive\r\nevent: signal\r\ndata: {\"name\":\"网络\",\r\ndata: \"age_ms\":0}\r\n\r\nevent: bye\ndata: {\"reason\":\"token_expired\"}\n\n";
        for width in 1..=wire.len() {
            let mut decoder = Decoder::default();
            let frames = wire
                .as_bytes()
                .chunks(width)
                .flat_map(|c| decoder.feed(c).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(frames.len(), 2);
            assert_eq!(frames[0].event, "signal");
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&frames[0].data).unwrap()["name"],
                "网络"
            );
            assert_eq!(frames[1].event, "bye");
        }
        assert!(Decoder::default().feed(&vec![b'x'; MAX_FRAME + 1]).is_err());
    }
}
