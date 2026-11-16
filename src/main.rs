//! volta — describe a circuit in words, get the drawing, the sketch and a
//! screenshot.
//!
//! The GUI is the default; the same engine answers without a display so an
//! agent can plan and render circuits too.

#[cfg(feature = "book-tasks")]
mod book_tasks;
mod checks;
mod compose;
mod config;
mod icons;
mod lessons;
mod manual;
mod netlist;
mod model;
mod modules;
mod parse;
mod paths;
mod png;
mod project;
mod scene;
mod spec;
mod style;
mod svg;
mod templates;
mod text;
mod theme;
mod ui;
mod words;

use std::path::PathBuf;

const HELP: &str = "\
volta — схема из текста: макетка, скетч, скриншот

  volta                         открыть окно
  volta plan <текст>            разобрать задачу и показать три варианта деталей
  volta render <текст> [ключи]  собрать схему, скетч и PNG без окна
  volta list                    что вообще умеет
  volta --help

Ключи для render:
  --choice N      вариант деталей, 0..2 (по умолчанию 0)
  --out DIR       куда писать файлы (по умолчанию ~/Pictures/volta)
  --lang ru|en    язык скетча и подписей (по умолчанию из Omarchy)
  --labels        подписать детали на скриншоте
  --wires arc|straight|ortho  как вести провода (по умолчанию из настроек)
  --json          машинный вывод
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        None | Some("open") => {
            if let Err(e) = ui::run() {
                eprintln!("volta: {e}");
                std::process::exit(1);
            }
        }
        Some("plan") => cmd_plan(&args[1..]),
        Some("render") => cmd_render(&args[1..]),
        Some("list") => cmd_list(),
        Some("help") | Some("-h") | Some("--help") => print!("{HELP}"),
        Some(other) => {
            eprintln!("volta: неизвестная команда «{other}»");
            eprint!("{HELP}");
            std::process::exit(2);
        }
    }
}

struct Flags {
    task: String,
    choice: usize,
    out: PathBuf,
    lang: text::Lang,
    labels: bool,
    wires: crate::model::WireStyle,
    json: bool,
}

fn flags(args: &[String]) -> Flags {
    let mut f = Flags {
        task: String::new(),
        choice: 0,
        out: png::default_dir(),
        lang: text::Lang::from_tag(&theme::language_tag()),
        labels: false,
        wires: crate::config::load_wire_style(),
        json: false,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--choice" => {
                i += 1;
                f.choice = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(0);
            }
            "--out" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    f.out = PathBuf::from(v);
                }
            }
            "--lang" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    f.lang = text::Lang::from_tag(v);
                }
            }
            "--labels" => f.labels = true,
            "--wires" => {
                i += 1;
                f.wires = match args.get(i).map(|s| s.as_str()) {
                    Some("arc") => crate::model::WireStyle::Arc,
                    Some("straight") => crate::model::WireStyle::Straight,
                    _ => crate::model::WireStyle::Ortho,
                };
            }
            "--json" => f.json = true,
            other => {
                if !other.starts_with("--") {
                    if !f.task.is_empty() {
                        f.task.push(' ');
                    }
                    f.task.push_str(other);
                }
            }
        }
        i += 1;
    }
    f
}

fn cmd_plan(args: &[String]) {
    let f = flags(args);
    if f.task.trim().is_empty() {
        eprintln!("volta plan: нужен текст задачи");
        std::process::exit(2);
    }
    let parsed = parse::parse(&f.task);
    let lang = f.lang;
    if let Some(msg) = parsed.unsupported {
        let msg = msg.get(lang).to_string();
        if f.json {
            println!(
                "{}",
                serde_json::json!({"ok": false, "error": msg, "understood": parsed.understood.iter().map(|l| l.get(lang)).collect::<Vec<_>>()})
            );
        } else {
            eprintln!("{msg}");
        }
        std::process::exit(1);
    }
    let Some(plan) = templates::plan(&parsed.intent, lang) else {
        let msg = parsed
            .unsupported
            .map(|l| l.get(lang).to_string())
            .unwrap_or_else(|| "задача не распознана".into());
        if f.json {
            println!(
                "{}",
                serde_json::json!({"ok": false, "error": msg, "understood": parsed.understood.iter().map(|l| l.get(lang)).collect::<Vec<_>>()})
            );
        } else {
            eprintln!("{msg}");
        }
        std::process::exit(1);
    };
    if f.json {
        let choices: Vec<_> = plan
            .choices
            .iter()
            .enumerate()
            .map(|(i, c)| {
                serde_json::json!({
                    "index": i,
                    "name": c.name.get(lang),
                    "blurb": c.blurb.get(lang),
                    "pros": c.pros.iter().map(|l| l.get(lang)).collect::<Vec<_>>(),
                    "cons": c.cons.iter().map(|l| l.get(lang)).collect::<Vec<_>>(),
                    "specs": c.specs.iter().map(|(k, v)| serde_json::json!([k.get(lang), v])).collect::<Vec<_>>(),
                    "level": c.level,
                    "recommended": i == plan.recommended,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "title": plan.title_text(lang),
                "understood": parsed.understood.iter().map(|l| l.get(lang)).collect::<Vec<_>>(),
                "choices": choices,
            })
        );
    } else {
        println!("{}", plan.title_text(lang));
        println!();
        for (i, c) in plan.choices.iter().enumerate() {
            let mark = if i == plan.recommended { "*" } else { " " };
            println!("{mark} [{i}] {}  — {}", c.name.get(lang), c.blurb.get(lang));
            for (k, v) in &c.specs {
                print!("      {}: {}   ", k.get(lang), v);
            }
            println!();
            for p in &c.pros {
                println!("      + {}", p.get(lang));
            }
            for p in &c.cons {
                println!("      − {}", p.get(lang));
            }
            println!();
        }
    }
}

fn cmd_render(args: &[String]) {
    let f = flags(args);
    let lang = f.lang;
    if f.task.trim().is_empty() {
        eprintln!("volta render: нужен текст задачи");
        std::process::exit(2);
    }
    let parsed = parse::parse(&f.task);
    if let Some(msg) = parsed.unsupported {
        let msg = msg.get(lang).to_string();
        if f.json {
            println!("{}", serde_json::json!({"ok": false, "error": msg}));
        } else {
            eprintln!("{msg}");
        }
        std::process::exit(1);
    }
    let Some(built) = templates::build(&parsed.intent, f.choice, lang) else {
        let msg = parsed
            .unsupported
            .map(|l| l.get(lang).to_string())
            .unwrap_or_else(|| "задача не распознана".into());
        if f.json {
            println!("{}", serde_json::json!({"ok": false, "error": msg}));
        } else {
            eprintln!("{msg}");
        }
        std::process::exit(1);
    };
    let report = checks::run(&built, lang);

    let th = theme::Theme::load();
    let sc = scene::draw(
        &built.circuit,
        &th,
        &scene::Opts {
            labels: f.labels,
            style: f.wires,
            bench: Some(scene::Rgba::rgb(0xEF, 0xEE, 0xEB)),
            ..scene::Opts::default()
        },
    );
    let svg_src = svg::render(&sc, 1.0);
    if let Err(e) = std::fs::create_dir_all(&f.out) {
        eprintln!("volta render: {}: {e}", f.out.display());
        std::process::exit(1);
    }
    let svg_path = f.out.join("circuit.svg");
    let png_path = f.out.join("circuit.png");
    let ino_path = f.out.join("sketch.ino");
    let json_path = f.out.join("report.json");
    if let Err(e) = std::fs::write(&svg_path, &svg_src) {
        eprintln!("volta render: {}: {e}", svg_path.display());
        std::process::exit(1);
    }
    let png_err = png::png_from_svg(&svg_src, &png_path, 2400).err();
    let _ = std::fs::write(&ino_path, &built.code);
    let serialised = serde_json::json!({
        "ok": report.failed() == 0,
        "title": built.title,
        "summary": built.summary.get(lang),
        "choice": built.choice,
        "parts": templates::bom(&built.circuit, lang).iter().map(|(v, k, n)| serde_json::json!({"value": v, "kind": k, "count": n})).collect::<Vec<_>>(),
        "checks": report.checks.iter().map(|c| serde_json::json!({
            "name": c.name.get(lang),
            "status": match c.sev { checks::Sev::Pass => "pass", checks::Sev::Warn => "warn", checks::Sev::Fail => "fail" },
            "detail": c.detail,
        })).collect::<Vec<_>>(),
        "notes": built.notes.iter().map(|l| l.get(lang)).collect::<Vec<_>>(),
        "files": {
            "svg": svg_path,
            "png": png_path,
            "sketch": ino_path,
            "report": json_path,
        },
    });
    let _ = std::fs::write(&json_path, serde_json::to_string_pretty(&serialised).unwrap_or_default());

    if f.json {
        println!("{}", serialised);
    } else {
        println!("{}", built.title);
        println!("{}", built.summary.get(lang));
        println!();
        for c in &report.checks {
            let mark = match c.sev {
                checks::Sev::Pass => "ok  ",
                checks::Sev::Warn => "warn",
                checks::Sev::Fail => "FAIL",
            };
            println!("{mark} {:<22} {}", c.name.get(lang), c.detail);
        }
        println!();
        println!("svg  {}", svg_path.display());
        println!("png  {}", png_path.display());
        println!("ino  {}", ino_path.display());
        if let Some(e) = png_err {
            eprintln!("png не сделан: {e}");
        }
    }
    if report.failed() > 0 {
        std::process::exit(1);
    }
}

fn cmd_list() {
    let lang = text::Lang::from_tag(&theme::language_tag());
    let rows = [
        (
            text::L::new("двигатель + кнопка", "motor + button"),
            text::L::new("ступени скорости, ШИМ", "speed steps over PWM"),
        ),
        (
            text::L::new("светодиод + кнопка", "LED + button"),
            text::L::new("вкл/выкл, в том числе RGB", "toggle, RGB included"),
        ),
        (
            text::L::new("светодиод", "LED"),
            text::L::new("мигание, бегущий огонь", "blink, chaser"),
        ),
        (
            text::L::new("светодиод + фоторезистор", "LED + photoresistor"),
            text::L::new("ночник с порогом", "night light with a threshold"),
        ),
        (
            text::L::new("сервопривод", "servo"),
            text::L::new("проход по углам, ручка", "sweep, knob control"),
        ),
        (
            text::L::new("звук", "sound"),
            text::L::new("пьезо, буззер, динамик", "piezo, buzzer, speaker"),
        ),
        (
            text::L::new("драйвер двигателей", "motor driver"),
            text::L::new("L298N или L293D, два мотора", "L298N or L293D, two motors"),
        ),
        (
            text::L::new("шаговый двигатель", "stepper"),
            text::L::new("28BYJ-48 с ULN2003", "28BYJ-48 with ULN2003"),
        ),
        (
            text::L::new("RFID", "RFID"),
            text::L::new("RC522: UID, экран, замок", "RC522: UID, screen, lock"),
        ),
        (
            text::L::new("радио 433 МГц", "433 MHz radio"),
            text::L::new("FS1000A и XY-MK-5V", "FS1000A and XY-MK-5V"),
        ),
        (
            text::L::new("NRF24L01", "NRF24L01"),
            text::L::new("обмен по 2.4 ГГц", "2.4 GHz link"),
        ),
        (
            text::L::new("Bluetooth", "Bluetooth"),
            text::L::new("HC-05/06 и телефон", "HC-05/06 and a phone"),
        ),
        (
            text::L::new("GSM", "GSM"),
            text::L::new("NEOWAY M590: SMS, звонки", "NEOWAY M590: SMS, calls"),
        ),
        (
            text::L::new("Ethernet", "Ethernet"),
            text::L::new("W5100 и ENC28J60, веб", "W5100 and ENC28J60, web"),
        ),
        (
            text::L::new("композиции", "compositions"),
            text::L::new(
                "16 датчиков → светодиод, пищалка, реле, экран, серво, мотор, RGB",
                "16 sensors → LED, buzzer, relay, display, servo, motor, RGB",
            ),
        ),
        (
            text::L::new("уроки из книг", "lessons from the books"),
            text::L::new(
                "51 урок и 10 занятий: датчики, экраны, память",
                "51 lessons and 10 exercises: sensors, screens, memory",
            ),
        ),
    ];
    for (a, b) in rows {
        println!("{:<34} {}", a.get(lang), b.get(lang));
    }
}
