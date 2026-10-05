pub struct Mail<'a> {
    pub app_name: &'a str,      // "Kreative Kompanion"
    pub app_url: &'a str,       // "https://kompanion.kreative-kompas.com" (no trailing slash), "" when unknown
    pub logo_url: &'a str,      // absolute URL of the logo, "" for none
    pub brand: &'a str,         // "#5c398e" (button and links; white text on it)
    pub status: Status,
    pub title: &'a str,         // the task title
    pub intro: &'a str,         // one sentence: what happened
    pub rows: &'a [(&'a str, &'a str)], // ("What's needed", "folder not found"), ("Project", "Kreative Kompanion")
    pub button: Option<(&'a str, &'a str)>, // ("Open the task", url)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Status { NeedsYou, Failed, Done, Info }

/// HTML-escapes & < > " ' (as &amp; &lt; &gt; &quot; &#39;).
pub fn esc(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The chip: (background, text, emoji + label). NeedsYou: ("#fde7cc", "#1c1329", "⏳ Needs you"); Failed: ("#ffd6e3", "#1c1329", "❌ Failed"); Done: ("#e6f6ea", "#1c1329", "✅ Done"); Info: ("#f4eefc", "#1c1329", "ℹ️ Update").
pub fn chip(s: Status) -> (&'static str, &'static str, &'static str) {
    match s {
        Status::NeedsYou => ("#fde7cc", "#1c1329", "⏳ Needs you"),
        Status::Failed => ("#ffd6e3", "#1c1329", "❌ Failed"),
        Status::Done => ("#e6f6ea", "#1c1329", "✅ Done"),
        Status::Info => ("#f4eefc", "#1c1329", "ℹ️ Update"),
    }
}

fn brand_or_default(b: &str) -> &str {
    if b.len() == 7 && b.starts_with('#') && b[1..].chars().all(|c| c.is_ascii_hexdigit()) {
        b
    } else {
        "#5c398e"
    }
}

pub fn render(m: &Mail) -> String {
    let mut html = String::new();

    // Header
    html.push_str("<!DOCTYPE html><html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta name=\"color-scheme\" content=\"light\"></head>\n");
    html.push_str("<body style=\"margin:0;padding:0;background:#f4eefc;\">\n");
    html.push_str("<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"background:#f4eefc;\"><tr><td align=\"center\" style=\"padding:24px 12px;\">\n");
    html.push_str("<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"max-width:640px;background:#ffffff;border-radius:10px;overflow:hidden;font-family:Inter,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;color:#1c1329;font-size:15px;line-height:1.5;\">\n");

    // Header row
    html.push_str("<tr><td style=\"background:#1c1329;padding:14px 24px;\">\n");
    html.push_str("  <table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\"><tr>\n");
    
    if !m.logo_url.is_empty() {
        html.push_str("    <td style=\"padding-right:10px;\"><img src=\"");
        html.push_str(&esc(m.logo_url));
        html.push_str("\" width=\"28\" height=\"28\" alt=\"\" style=\"display:block;border:0;\"></td>\n");
    }
    
    html.push_str("    <td style=\"color:#f4eefc;font-size:15px;font-weight:600;\">");
    html.push_str(&esc(m.app_name));
    html.push_str("</td>\n");
    html.push_str("  </tr></table>\n");
    html.push_str("</td></tr>\n");

    // Orange rule row
    html.push_str("<tr><td style=\"height:4px;background:#f3941f;font-size:0;line-height:0;\">&nbsp;</td></tr>\n");

    // Body row
    html.push_str("<tr><td class=\"kk-body\" style=\"padding:24px;\">\n");

    // Chip
    let (bg, fg, label) = chip(m.status);
    html.push_str("<div style=\"display:inline-block;background:");
    html.push_str(bg);
    html.push_str(";color:");
    html.push_str(fg);
    html.push_str(";border-radius:999px;padding:4px 12px;font-weight:600;font-size:13px;\">");
    html.push_str(label);
    html.push_str("</div>\n");

    // Title
    html.push_str("<h1 style=\"margin:14px 0 4px;font-size:21px;line-height:1.3;color:#1c1329;\">");
    html.push_str(&esc(m.title));
    html.push_str("</h1>\n");

    // Intro
    html.push_str("<p style=\"margin:0 0 16px;color:#5e5873;\">");
    html.push_str(&esc(m.intro));
    html.push_str("</p>\n");

    // Rows table
    if !m.rows.is_empty() {
        html.push_str("<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" style=\"width:100%;font-size:14px;border-collapse:collapse;\">\n");
        for (i, (label, value)) in m.rows.iter().enumerate() {
            let line = if i > 0 { "border-top:1px solid #efe8f8;" } else { "" };
            html.push_str(&format!("<tr>\n  <td style=\"padding:6px 0;color:#5e5873;width:130px;{line}\">{}</td>\n", esc(label)));
            html.push_str(&format!("  <td style=\"padding:6px 0;{line}\">{}</td>\n</tr>\n", esc(value)));
        }
        html.push_str("</table>\n");
    }

    // Button
    if let Some((label, url)) = m.button {
        html.push_str("<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" style=\"margin:20px 0 4px;\">\n");
        html.push_str("<tr><td style=\"background:");
        
        let bg_color = brand_or_default(m.brand);
        
        html.push_str(bg_color);
        html.push_str(";border-radius:6px;\"><a href=\"");
        html.push_str(&esc(url));
        html.push_str("\" style=\"display:inline-block;padding:10px 20px;color:#ffffff;text-decoration:none;font-weight:600;\">");
        html.push_str(&esc(label));
        html.push_str("</a></td></tr>\n");
        html.push_str("</table>\n");
    }

    html.push_str("</td></tr>\n");

    // Footer row
    html.push_str("<tr><td style=\"padding:14px 24px;background:#f4eefc;color:#5e5873;font-size:12px;border-top:1px solid #e3d9f2;\">");
    html.push_str(&esc(m.app_name));
    
    // Write " · " and the link only when app_url is not empty
    if !m.app_url.is_empty() {
        html.push_str(" · ");
        html.push_str("<a href=\"");
        html.push_str(&esc(m.app_url));
        html.push_str("\" style=\"color:");
        let link_color = brand_or_default(m.brand);
        html.push_str(link_color);
        html.push_str("\">");
        html.push_str(&esc(m.app_url));
        html.push_str("</a>");
    }
    
    html.push_str("<br>You get this mail because of your notification settings in ");
    html.push_str(&esc(m.app_name));
    html.push_str(" (Settings &gt; Notifications).</td></tr>\n");

    html.push_str("</table>\n");
    html.push_str("</td></tr></table>\n</body></html>");

    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_esc() {
        assert_eq!(esc("<a&b>\"'"), "&lt;a&amp;b&gt;&quot;&#39;");
    }

    #[test]
    fn test_render_script_escape() {
        let m = Mail {
            app_name: "Test",
            app_url: "",
            logo_url: "",
            brand: "#5c398e",
            status: Status::Info,
            title: "<script>",
            intro: "Test intro",
            rows: &[("Key", "Value")],
            button: None,
        };
        let html = render(&m);
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn test_needs_you_chip() {
        let m = Mail {
            app_name: "Test",
            app_url: "",
            logo_url: "",
            brand: "#5c398e",
            status: Status::NeedsYou,
            title: "Task",
            intro: "Intro",
            rows: &[],
            button: None,
        };
        let html = render(&m);
        assert!(html.contains("Needs you"));
        assert!(html.contains("#fde7cc"));
    }

    #[test]
    fn test_bad_brand_fallback() {
        let m = Mail {
            app_name: "Test",
            app_url: "",
            logo_url: "",
            brand: "red",
            status: Status::Info,
            title: "Task",
            intro: "Intro",
            rows: &[],
            button: Some(("Click", "http://example.com")),
        };
        let html = render(&m);
        // Should use fallback color in button background
        assert!(html.contains("background:#5c398e;"));
        assert!(html.contains("href=\"http://example.com\""));
    }

    #[test]
    fn test_no_logo() {
        let m = Mail {
            app_name: "Test",
            app_url: "",
            logo_url: "",
            brand: "#5c398e",
            status: Status::Info,
            title: "Task",
            intro: "Intro",
            rows: &[],
            button: None,
        };
        let html = render(&m);
        assert!(!html.contains("<img"));
    }

    #[test]
    fn test_rows_escaped() {
        let m = Mail {
            app_name: "Test",
            app_url: "",
            logo_url: "",
            brand: "#5c398e",
            status: Status::Info,
            title: "Task",
            intro: "Intro",
            rows: &[("What's needed", "folder not found")],
            button: None,
        };
        let html = render(&m);
        assert!(html.contains("What&#39;s needed"));
        assert!(html.contains("folder not found"));
    }

    #[test]
    fn test_brand_red() {
        let m = Mail {
            app_name: "Test",
            app_url: "",
            logo_url: "",
            brand: "red",
            status: Status::Info,
            title: "Task",
            intro: "Intro",
            rows: &[],
            button: Some(("Open", "https://k.example/")),
        };
        let html = render(&m);
        assert!(html.contains("background:#5c398e;"));
    }

    #[test]
    fn test_brand_custom_hex() {
        let m = Mail {
            app_name: "Test",
            app_url: "",
            logo_url: "",
            brand: "#123abc",
            status: Status::Info,
            title: "Task",
            intro: "Intro",
            rows: &[],
            button: Some(("Open", "https://k.example/")),
        };
        let html = render(&m);
        assert!(html.contains("background:#123abc;"));
    }
}
