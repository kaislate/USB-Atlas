//! Self-contained HTML report.

use crate::descriptors::DescNode;
use crate::details::{self, hexdump, Section};
use crate::insights::{self, Severity};
use crate::model::Snapshot;
use crate::tree::{self, NodeKind};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn anchor(id: &str) -> String {
    let mut a: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    a.insert_str(0, "n-");
    a
}

const CSS: &str = r#"
:root{--bg:#f4f5f8;--card:#fff;--border:#e1e4eb;--text:#161a22;--muted:#5b6373;--faint:#9aa1ae;--accent:#4f5bd5;--ok:#16a34a;--warn:#d97706;--err:#dc2626;--info:#2563eb}
@media (prefers-color-scheme:dark){:root{--bg:#0e1014;--card:#191d24;--border:#272d37;--text:#e8ebf1;--muted:#969ead;--faint:#5e6674;--accent:#8b9cff;--ok:#4ade80;--warn:#fbbf24;--err:#f87171;--info:#60a5fa}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px/1.5 "Segoe UI",system-ui,sans-serif}
header{padding:24px 32px;border-bottom:1px solid var(--border);background:var(--card)}h1{margin:0;font-size:22px}header p{margin:4px 0 0;color:var(--muted)}
.layout{display:grid;grid-template-columns:minmax(260px,380px) 1fr;gap:0;min-height:calc(100vh - 90px)}
nav{border-right:1px solid var(--border);padding:16px 12px;position:sticky;top:0;align-self:start;max-height:100vh;overflow:auto;background:var(--card)}
nav a{display:block;padding:3px 8px;border-radius:6px;color:var(--text);text-decoration:none;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-size:13px}
nav a:hover{background:var(--bg)}nav .empty{color:var(--faint)}nav .err{color:var(--err)}
main{padding:24px 32px;min-width:0}
.node{background:var(--card);border:1px solid var(--border);border-radius:14px;padding:18px 22px;margin-bottom:18px}
.node h2{margin:0 0 2px;font-size:18px}.kind{font-size:11px;letter-spacing:.06em;text-transform:uppercase;color:var(--faint);font-weight:600}
h3{font-size:14px;margin:16px 0 6px;color:var(--accent)}
table{border-collapse:collapse;width:100%}td{padding:3px 10px 3px 0;vertical-align:top}td:first-child{color:var(--muted);width:230px}
.mono{font-family:"Cascadia Mono",Consolas,monospace;font-size:12.5px}
.flag-error{color:var(--err)}.flag-warning{color:var(--warn)}.flag-info{color:var(--info)}
details{margin:4px 0 4px 14px}summary{cursor:pointer;font-weight:600}
pre{background:var(--bg);border:1px solid var(--border);border-radius:8px;padding:10px;overflow:auto}
.pill{display:inline-block;padding:1px 9px;border-radius:99px;font-size:12px;background:var(--bg);border:1px solid var(--border);margin-right:4px}
.insight{border-left:3px solid var(--warn);padding:6px 12px;margin:6px 0;background:var(--bg);border-radius:0 8px 8px 0}
.insight.error{border-color:var(--err)}.insight.info{border-color:var(--info)}
@media (max-width:800px){.layout{grid-template-columns:1fr}nav{position:static;max-height:none;border-right:0;border-bottom:1px solid var(--border)}}
"#;

fn desc_html(out: &mut String, d: &DescNode, depth: usize) {
    if depth > 0 {
        out.push_str(&format!(
            "<details open><summary>{}</summary>",
            esc(&d.title)
        ));
    }
    out.push_str("<table class=\"mono\">");
    for f in &d.fields {
        out.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            esc(&f.name),
            esc(&f.display)
        ));
    }
    out.push_str("</table>");
    for w in &d.warnings {
        out.push_str(&format!("<div class=\"flag-warning\">⚠ {}</div>", esc(w)));
    }
    for c in &d.children {
        desc_html(out, c, depth + 1);
    }
    if depth > 0 {
        out.push_str("</details>");
    }
}

fn sections_html(out: &mut String, secs: &[Section], hexdumps: bool) {
    for s in secs {
        out.push_str(&format!("<h3>{}</h3>", esc(&s.title)));
        if !s.rows.is_empty() {
            out.push_str("<table>");
            for r in &s.rows {
                let cls = match r.flag {
                    Some(Severity::Error) => " class=\"flag-error\"",
                    Some(Severity::Warning) => " class=\"flag-warning\"",
                    Some(Severity::Info) => " class=\"flag-info\"",
                    None => "",
                };
                out.push_str(&format!(
                    "<tr><td>{}</td><td{cls}>{}</td></tr>",
                    esc(&r.key),
                    esc(&r.value).replace('\n', "<br>")
                ));
            }
            out.push_str("</table>");
        }
        if let Some(d) = &s.desc {
            desc_html(out, d, 0);
        }
        if hexdumps {
            if let Some(raw) = &s.raw {
                out.push_str(&format!("<pre class=\"mono\">{}</pre>", esc(&hexdump(raw))));
            }
        }
    }
}

pub fn html_report(snap: &Snapshot, hexdumps: bool) -> String {
    let flat = tree::flatten(
        snap,
        &tree::FlattenOptions {
            show_empty_ports: true,
            show_child_devices: false,
            physical: false,
        },
    );
    let ins = insights::analyze(snap, &flat);
    let mut out = String::new();
    out.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
    out.push_str(&format!(
        "<title>USB report – {}</title><style>{CSS}</style></head><body>",
        esc(&snap.computer.name)
    ));
    out.push_str(&format!(
        "<header><h1>USB topology – {}</h1><p>{} · {} · generated by {} {}</p></header>",
        esc(&snap.computer.name),
        esc(&snap.computer.os),
        esc(&snap.taken_at),
        crate::app::APP_NAME,
        env!("CARGO_PKG_VERSION")
    ));
    out.push_str("<div class=\"layout\"><nav>");
    for n in &flat.nodes {
        let cls = if n.kind == NodeKind::EmptyPort {
            "empty"
        } else if n.health == tree::Health::Error {
            "err"
        } else {
            ""
        };
        let pad = n.depth * 14 + 8;
        let port = if n.port_label.is_empty() {
            String::new()
        } else {
            format!(
                "<span class=\"mono\" style=\"color:var(--faint)\">{}</span> ",
                esc(n.port_label.trim_start_matches("Port "))
            )
        };
        if n.kind == NodeKind::EmptyPort {
            out.push_str(&format!(
                "<a class=\"{cls}\" style=\"padding-left:{pad}px\">{port}{}</a>",
                esc(&n.label)
            ));
        } else {
            out.push_str(&format!(
                "<a class=\"{cls}\" style=\"padding-left:{pad}px\" href=\"#{}\">{port}{}</a>",
                anchor(&n.id),
                esc(&n.label)
            ));
        }
    }
    out.push_str("</nav><main>");
    if !ins.is_empty() {
        out.push_str("<section class=\"node\"><div class=\"kind\">Insights</div>");
        for i in &ins {
            let cls = match i.severity {
                Severity::Error => "error",
                Severity::Warning => "",
                Severity::Info => "info",
            };
            out.push_str(&format!(
                "<div class=\"insight {cls}\"><a href=\"#{}\"><b>{}</b></a><br><span style=\"color:var(--muted)\">{}</span></div>",
                anchor(&i.node_id),
                esc(&i.title),
                esc(&i.detail).replace('\n', "<br>")
            ));
        }
        out.push_str("</section>");
    }
    for n in &flat.nodes {
        if n.kind == NodeKind::EmptyPort {
            continue;
        }
        let kind = match n.kind {
            NodeKind::Computer => "Computer",
            NodeKind::Controller => "Host controller",
            NodeKind::RootHub => "Root hub",
            NodeKind::Hub => "Hub",
            _ => "Device",
        };
        out.push_str(&format!(
            "<section class=\"node\" id=\"{}\"><div class=\"kind\">{kind}</div><h2>{}</h2>",
            anchor(&n.id),
            esc(&n.label)
        ));
        if matches!(n.kind, NodeKind::Device | NodeKind::Hub) {
            out.push_str(&format!(
                "<span class=\"pill\">{}</span>",
                esc(n.speed.label())
            ));
            if !n.port_label.is_empty() {
                out.push_str(&format!(
                    "<span class=\"pill\">{}</span>",
                    esc(&n.port_label)
                ));
            }
        }
        sections_html(&mut out, &details::sections(snap, &n.path), hexdumps);
        out.push_str("</section>");
    }
    out.push_str("</main></div></body></html>");
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn html_report_is_well_formed_enough() {
        let s = crate::demo::snapshot();
        let h = super::html_report(&s, true);
        assert!(h.starts_with("<!doctype html>"));
        assert!(h.contains("Ultra Fit"));
        assert!(h.ends_with("</html>"));
        assert_eq!(
            h.matches("<section").count(),
            h.matches("</section>").count()
        );
    }
}
