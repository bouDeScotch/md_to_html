use clap::Parser;
use notify::{RecursiveMode, Watcher, recommended_watcher};
use regex::Regex;
use std::fs;
use std::io::Read;
use std::iter::zip;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use tiny_http::{Header, Response, Server};

const LIVE_RELOAD_SCRIPT: &str = r#"
<script>
    const es = new EventSource('/reload');
    es.onmessage = () => location.reload();
</script>
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Normal,
    OrderedList,
    UnorderedList,
    Code,
}

#[derive(Parser)]
#[command(name = "md_to_html", about = "converts markdown file into HTML")]
struct Args {
    input: String,
    output: String,
    #[arg(short, long)]
    watch: bool,
    #[arg(short, long)]
    config: Option<String>,
}

fn parse_file(path: &str) -> std::io::Result<Vec<String>> {
    let mut file = fs::File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();
    for character in contents.chars() {
        if character != '\n' {
            current_line.push(character);
        } else {
            lines.push(current_line);
            current_line = String::new();
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    Ok(lines)
}

fn parse_headings(line: &str) -> String {
    let line = line.trim_end_matches('\r');

    // Check for heading
    for i in (1..=6).rev() {
        let mut identifier = String::new();
        for _ in 1..=i {
            identifier.push('#');
        }
        identifier.push(' ');
        if line.starts_with(&identifier) {
            return format!("<h{0}>{1}</h{0}>", i, &line[(i + 1)..]);
        }
    }
    return line.to_string();
}

fn parse_lists(line: &str) -> String {
    let mut line = line.trim_end_matches('\r').to_string();

    if line.starts_with("- ") {
        line = format!("<li>{}</li>", &line[2..]);
    };

    let mut chars = line.chars().enumerate();
    while let Some((i, c)) = chars.next() {
        if c.is_alphanumeric() {
            continue;
        } else if c == '.' && i > 0 {
            if let Some((_, ' ')) = chars.next() {
                line = format!("<li>{}</li>", &line[(i + 2)..]);
            }
            break;
        } else {
            break;
        }
    }
    line.to_string()
}

fn parse_line(line: &str, regexes: &[Regex], htmls: &[&str]) -> String {
    let line = parse_headings(line);
    let mut line = parse_lists(&line);

    for (regex, html_form) in zip(regexes, htmls) {
        line = regex.replace_all(&line, *html_form).to_string();
    }
    if line.starts_with("```") {
        line = String::new();
    }
    line
}

fn classify_lines(lines: &Vec<String>) -> Vec<State> {
    let mut states = Vec::new();
    states.resize(lines.len(), State::Normal);
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("- ") {
            states[i] = State::UnorderedList;
        };
    }

    for (line_index, line) in lines.iter().enumerate() {
        let mut chars = line.chars().enumerate();
        while let Some((i, c)) = chars.next() {
            if c.is_alphanumeric() {
                continue;
            } else if c == '.' && i > 0 {
                if let Some((_, ' ')) = chars.next() {
                    states[line_index] = State::OrderedList;
                }
                break;
            } else {
                break;
            }
        }
    }

    let mut inside_code_block: bool = false;
    for (line_index, line) in lines.iter().enumerate() {
        if line.starts_with("```") {
            if inside_code_block {
                inside_code_block = !inside_code_block;
                continue;
            };
            inside_code_block = !inside_code_block;
        };

        if inside_code_block {
            states[line_index] = State::Code;
        }
    }
    states
}

fn convert_lines(lines: &Vec<String>, states: &Vec<State>) -> String {
    let mut text = String::new();

    let mut previous_state = State::Normal;
    let mut in_paragraph = false;
    for (i, line) in lines.iter().enumerate() {
        let mut line = line.clone();
        let state = states[i];
        let is_plain = state == State::Normal && !line.is_empty() && !line.starts_with('<');

        if is_plain {
            if !in_paragraph {
                text.push_str("<p>");
                in_paragraph = true;
            } else {
                text.push(' ');
            }
            text.push_str(&line);
            continue;
        } else if in_paragraph {
            text.push_str("</p>\n");
            in_paragraph = false;
        }

        if previous_state != state {
            match state {
                State::OrderedList => {
                    line = format!("<ol>\n{}", line);
                }
                State::UnorderedList => {
                    line = format!("<ul>\n{}", line);
                }
                State::Code => {
                    line = format!("<pre><code>");
                }
                State::Normal => match previous_state {
                    State::OrderedList => {
                        line = format!("{}\n</ol>", line);
                    }
                    State::UnorderedList => {
                        line = format!("{}\n</ul>", line);
                    }
                    State::Code => {
                        line = format!("</code></pre>");
                    }
                    State::Normal => {}
                },
            }
        };
        previous_state = state;

        text.push_str(&line);
        text.push('\n');
    }

    if in_paragraph {
        text.push_str("</p>");
    };

    text
}

fn wrap_html(body: &str, title: &str, style: &str) -> String {
    format!(
        "<!DOCTYPE html><head><style>{}</style><title>{}</title></head><body>{}{}</body>",
        style, title, body, LIVE_RELOAD_SCRIPT
    )
}

fn convert(input_path: &String, output_path: &String, style:&str) {
    let mut file_lines = parse_file(input_path).expect("Error during file parsing");

    let regexes = [
        Regex::new(r"\*\*(.*?)\*\*").unwrap(),
        Regex::new(r"\*(.*?)\*").unwrap(),
        Regex::new(r"\[(.*?)\]\((.*?)\)").unwrap(),
        Regex::new(r"^---$").unwrap(),
        Regex::new(r"`(.*?)`").unwrap(),
    ];
    let htmls = [
        "<strong>$1</strong>",
        "<em>$1</em>",
        r#"<a href="$2">$1</a>"#,
        "<hr/>",
        "<code>$1</code>",
    ];

    let line_states = classify_lines(&file_lines);
    for (i, line) in &mut file_lines.iter_mut().enumerate() {
        if line_states[i] != State::Code {
            *line = parse_line(line, &regexes, &htmls);
        }
    }
    let text = convert_lines(&file_lines, &line_states);
    let text = wrap_html(&text, input_path, style);
    fs::write(output_path, &text).expect("Error writing to the file");
    //println!("{}", &text);
}

fn start_server(output_path: String, reload_flag: Arc<AtomicBool>) {
    let server = Arc::new(Server::http("0.0.0.0:8080").expect("Failed to start server"));
    println!("Serving on http://localhost:8080");

    for request in server.incoming_requests() {
        let reload_flag = reload_flag.clone();
        let output_path = output_path.clone();

        thread::spawn(move || {
            let url = request.url().to_string();

            if url == "/reload" {
                loop {
                    if reload_flag.load(Ordering::Relaxed) {
                        reload_flag.store(false, Ordering::Relaxed);
                        let _ = request.respond(
                            Response::from_string("data: reload\n\n")
                                .with_header("Content-Type: text/event-stream".parse::<Header>().unwrap())
                                .with_header("Cache-Control: no-cache".parse::<Header>().unwrap()),
                        );
                        break;
                    }
                    thread::sleep(std::time::Duration::from_millis(100));
                }
            } else {
                let html = fs::read_to_string(&output_path)
                    .unwrap_or_else(|_| "<p>File not found</p>".to_string());
                let _ = request.respond(
                    Response::from_string(html)
                        .with_header("Content-Type: text/html".parse::<Header>().unwrap()),
                );
            }
        });
    }
}

// In your main function, replace open::that() with this:
fn start_live_server(output_path: &str) -> Arc<AtomicBool> {
    let reload_flag = Arc::new(AtomicBool::new(false));
    let reload_flag_clone = reload_flag.clone();
    let output_path = output_path.to_string();

    thread::spawn(move || {
        start_server(output_path, reload_flag_clone);
    });

    reload_flag
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let style = match &args.config {
        Some(path) => fs::read_to_string(path).expect("Couldn't read config file."),
        None => include_str!("style.css").to_string()
    };

    convert(&args.input, &args.output, &style);

    if !args.watch {
        return Ok(())
    };
    let input = args.input.clone();
    let output = args.output.clone();

    let reload_flag = start_live_server(&args.output);

    let reload_flag_clone = reload_flag.clone();
    let mut watcher = recommended_watcher(move |res| match res {
        Ok(_) => {
            println!("File changed, reconverting...");
            convert(&input, &output, &style);
            reload_flag_clone.store(true, Ordering::Relaxed);
        }
        Err(e) => eprintln!("Watch error {:?}", e),
    })?;

    watcher.watch(Path::new(&args.input), RecursiveMode::NonRecursive)?;

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
