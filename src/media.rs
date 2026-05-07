use grammers_client::types::{media::Document, Media, Message};

pub fn is_downloadable(media: &Media) -> bool {
    matches!(media, Media::Photo(_) | Media::Document(_) | Media::Sticker(_))
}

pub fn matches_filter(media: &Media, filter: &str) -> bool {
    match filter {
        "all" => true,
        "photo" => matches!(media, Media::Photo(_)),
        "video" => match media {
            Media::Document(doc) => doc
                .mime_type()
                .map(|m| m.starts_with("video/"))
                .unwrap_or(false),
            _ => false,
        },
        "audio" => match media {
            Media::Document(doc) => doc
                .mime_type()
                .map(|m| m.starts_with("audio/"))
                .unwrap_or(false),
            _ => false,
        },
        "document" => matches!(media, Media::Document(_)),
        "sticker" => matches!(media, Media::Sticker(_)),
        _ => true,
    }
}

pub fn media_filename(media: &Media, message: &Message) -> String {
    match media {
        Media::Photo(_) => format!("photo_{}.jpg", message.id()),
        Media::Document(doc) => {
            let name = doc.name();
            if !name.is_empty() {
                name.to_string()
            } else {
                let ext = doc.mime_type().and_then(mime_to_ext).unwrap_or("bin");
                format!("{}_{}.{}", doc_kind(doc), message.id(), ext)
            }
        }
        Media::Sticker(_) => format!("sticker_{}.webp", message.id()),
        _ => format!("file_{}", message.id()),
    }
}

pub fn media_size(media: &Media) -> u64 {
    match media {
        Media::Photo(p) => p.size() as u64,
        Media::Document(d) => d.size() as u64,
        _ => 0,
    }
}

fn doc_kind(doc: &Document) -> &'static str {
    match doc.mime_type() {
        Some(m) if m.starts_with("video/") => "video",
        Some(m) if m.starts_with("audio/") => "audio",
        _ => "document",
    }
}

pub fn mime_to_ext(mime: &str) -> Option<&'static str> {
    match mime {
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        "video/x-matroska" => Some("mkv"),
        "video/quicktime" => Some("mov"),
        "audio/mpeg" => Some("mp3"),
        "audio/ogg" => Some("ogg"),
        "audio/flac" => Some("flac"),
        "audio/mp4" => Some("m4a"),
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "application/pdf" => Some("pdf"),
        "application/zip" => Some("zip"),
        "application/vnd.rar" | "application/x-rar-compressed" => Some("rar"),
        "application/x-7z-compressed" => Some("7z"),
        _ => None,
    }
}

pub fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}
