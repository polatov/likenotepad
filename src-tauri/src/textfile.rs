// textfile.rs — reading and writing text files the way Notepad does: a file is saved back
// in the encoding and with the line endings it was opened with.
//
// The editor (a <textarea>) only ever holds "\n" line endings, so the file's own line
// ending and encoding are remembered per path (FORMATS) when it is read and applied again
// when it is written.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Windows1251,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eol {
    Lf,
    CrLf,
    Cr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    pub encoding: Encoding,
    pub bom: bool,
    pub eol: Eol,
}

impl Default for Format {
    // New documents: UTF-8 without a BOM and "\n", as before.
    fn default() -> Self {
        Format { encoding: Encoding::Utf8, bom: false, eol: Eol::Lf }
    }
}

/// Decodes a file's bytes into editor text ("\n" line endings, no BOM) and its format.
/// A BOM decides the encoding; otherwise valid UTF-8 is UTF-8 and anything else is read
/// as Windows-1251. The most frequent line ending in the file becomes its line ending.
pub fn decode(bytes: &[u8]) -> (String, Format) {
    let (encoding, bom_len) = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        (Encoding::Utf8, 3)
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        (Encoding::Utf16Le, 2)
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        (Encoding::Utf16Be, 2)
    } else if std::str::from_utf8(bytes).is_ok() {
        (Encoding::Utf8, 0)
    } else {
        (Encoding::Windows1251, 0)
    };
    let body = &bytes[bom_len..];
    let raw = match encoding {
        Encoding::Utf8 => String::from_utf8_lossy(body).into_owned(),
        Encoding::Utf16Le => encoding_rs::UTF_16LE.decode_without_bom_handling(body).0.into_owned(),
        Encoding::Utf16Be => encoding_rs::UTF_16BE.decode_without_bom_handling(body).0.into_owned(),
        Encoding::Windows1251 => encoding_rs::WINDOWS_1251.decode_without_bom_handling(body).0.into_owned(),
    };

    let crlf = raw.matches("\r\n").count();
    let cr = raw.matches('\r').count() - crlf;
    let lf = raw.matches('\n').count() - crlf;
    let eol = if crlf > 0 && crlf >= lf && crlf >= cr {
        Eol::CrLf
    } else if cr > lf {
        Eol::Cr
    } else {
        Eol::Lf
    };
    let text = raw.replace("\r\n", "\n").replace('\r', "\n");
    (text, Format { encoding, bom: bom_len > 0, eol })
}

/// Encodes editor text ("\n" line endings) in the given format. Text that Windows-1251
/// cannot hold (emoji, other scripts) is written as UTF-8 instead, so nothing is lost;
/// the format actually used is returned.
pub fn encode(text: &str, format: Format) -> (Vec<u8>, Format) {
    let text = match format.eol {
        Eol::Lf => text.to_string(),
        Eol::CrLf => text.replace('\n', "\r\n"),
        Eol::Cr => text.replace('\n', "\r"),
    };
    match format.encoding {
        Encoding::Utf8 => {
            let mut out = if format.bom { vec![0xEF, 0xBB, 0xBF] } else { Vec::new() };
            out.extend_from_slice(text.as_bytes());
            (out, format)
        }
        Encoding::Utf16Le | Encoding::Utf16Be => {
            let le = format.encoding == Encoding::Utf16Le;
            let mut out = if format.bom { if le { vec![0xFF, 0xFE] } else { vec![0xFE, 0xFF] } } else { Vec::new() };
            for unit in text.encode_utf16() {
                out.extend_from_slice(&if le { unit.to_le_bytes() } else { unit.to_be_bytes() });
            }
            (out, format)
        }
        Encoding::Windows1251 => {
            let (bytes, _, unmappable) = encoding_rs::WINDOWS_1251.encode(&text);
            if unmappable {
                encode(&text.replace("\r\n", "\n").replace('\r', "\n"), Format { encoding: Encoding::Utf8, bom: false, ..format })
            } else {
                (bytes.into_owned(), format)
            }
        }
    }
}

fn formats() -> &'static Mutex<HashMap<String, Format>> {
    static FORMATS: OnceLock<Mutex<HashMap<String, Format>>> = OnceLock::new();
    FORMATS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The format a path was last read or written with (default for unknown paths).
pub fn format_of(path: &str) -> Format {
    formats().lock().unwrap().get(path).copied().unwrap_or_default()
}

pub fn remember(path: &str, format: Format) {
    formats().lock().unwrap().insert(path.to_string(), format);
}

/// Reads a file into editor text and remembers its format.
pub fn read(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let (text, format) = decode(&bytes);
    remember(path, format);
    Ok(text)
}

/// Writes editor text to `path` in `format` (see `encode`) and remembers the format used.
pub fn write(path: &str, text: &str, format: Format) -> Result<(), String> {
    let (bytes, used) = encode(text, format);
    std::fs::write(path, bytes).map_err(|e| e.to_string())?;
    remember(path, used);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(bytes: &[u8]) -> Vec<u8> {
        let (text, format) = decode(bytes);
        encode(&text, format).0
    }

    #[test]
    fn utf8_lf_unchanged() {
        let b = "Привет\nмир\n".as_bytes();
        assert_eq!(decode(b), ("Привет\nмир\n".into(), Format::default()));
        assert_eq!(roundtrip(b), b);
    }

    #[test]
    fn crlf_is_kept() {
        let b = b"one\r\ntwo\r\n";
        let (text, format) = decode(b);
        assert_eq!(text, "one\ntwo\n");
        assert_eq!(format.eol, Eol::CrLf);
        assert_eq!(roundtrip(b), b);
    }

    #[test]
    fn old_mac_cr_is_kept() {
        let b = b"one\rtwo\r";
        assert_eq!(decode(b).1.eol, Eol::Cr);
        assert_eq!(roundtrip(b), b);
    }

    #[test]
    fn utf8_bom_is_kept_and_hidden_from_the_editor() {
        let b = b"\xEF\xBB\xBFtext";
        let (text, format) = decode(b);
        assert_eq!(text, "text");
        assert!(format.bom);
        assert_eq!(roundtrip(b), b);
    }

    #[test]
    fn utf16_both_orders() {
        let le = b"\xFF\xFEh\x00i\x00\r\x00\n\x00";
        let (text, format) = decode(le);
        assert_eq!(text, "hi\n");
        assert_eq!((format.encoding, format.eol), (Encoding::Utf16Le, Eol::CrLf));
        assert_eq!(roundtrip(le), le);
        let be = b"\xFE\xFF\x00h\x00i";
        assert_eq!(decode(be).0, "hi");
        assert_eq!(roundtrip(be), be);
    }

    #[test]
    fn windows_1251_is_kept() {
        let b = b"\xcf\xf0\xe8\xe2\xe5\xf2\r\n"; // «Привет» in Windows-1251, CRLF
        let (text, format) = decode(b);
        assert_eq!(text, "Привет\n");
        assert_eq!(format.encoding, Encoding::Windows1251);
        assert_eq!(roundtrip(b), b);
    }

    #[test]
    fn windows_1251_falls_back_to_utf8_when_text_does_not_fit() {
        let format = Format { encoding: Encoding::Windows1251, bom: false, eol: Eol::CrLf };
        let (bytes, used) = encode("Привет 🙂\n", format);
        assert_eq!(used, Format { encoding: Encoding::Utf8, bom: false, eol: Eol::CrLf });
        assert_eq!(bytes, "Привет 🙂\r\n".as_bytes());
    }

    #[test]
    fn mixed_endings_take_the_majority() {
        assert_eq!(decode(b"a\r\nb\r\nc\nd").1.eol, Eol::CrLf);
        assert_eq!(decode(b"a\nb\nc\r\nd").1.eol, Eol::Lf);
    }
}
