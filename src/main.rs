use crossterm::event::Event;
use crossterm::terminal as term;
use crossterm::{event, execute};
use handlebars::{handlebars_helper, Handlebars};
use shellwords;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use termion::get_tty;
use tui::backend::CrosstermBackend;
use tui::Terminal;
use xplr::app;
use xplr::error::Error;
use xplr::explorer;
use xplr::input::Key;
use xplr::ui;

handlebars_helper!(shellescape: |v: str| format!("{}", shellwords::escape(v)));
handlebars_helper!(readfile: |v: str| fs::read_to_string(v).unwrap_or_default());

fn main() -> Result<(), Error> {
    let mut pwd = PathBuf::from(env::args().skip(1).next().unwrap_or(".".into()))
        .canonicalize()
        .unwrap_or_default();

    let mut focused_path = None;

    if pwd.is_file() {
        focused_path = pwd.file_name().map(|n| n.to_string_lossy().to_string());
        pwd = pwd.parent().map(|p| p.into()).unwrap_or_default();
    }

    let pwd = pwd.to_string_lossy().to_string();

    let mut last_pwd = pwd.clone();

    let mut app = app::App::new(pwd.clone());

    let mut hb = Handlebars::new();
    hb.register_helper("shellescape", Box::new(shellescape));
    hb.register_helper("readfile", Box::new(readfile));
    hb.register_template_string(
        app::TEMPLATE_TABLE_ROW,
        &app.config()
            .general
            .table
            .row
            .cols
            .iter()
            .map(|c| c.format.to_string())
            .collect::<Vec<String>>()
            .join("\t"),
    )?;

    let mut result = Ok(());
    let mut output = None;

    let (tx_key, rx) = mpsc::channel();
    let tx_init = tx_key.clone();
    let tx_pipe = tx_key.clone();
    let tx_explorer = tx_key.clone();

    term::enable_raw_mode().unwrap();
    let mut stdout = get_tty().unwrap();
    // let mut stdout = stdout.lock();
    execute!(stdout, term::EnterAlternateScreen).unwrap();
    // let stdout = MouseTerminal::from(stdout);
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.hide_cursor()?;

    let (tx, rx_key) = mpsc::channel();
    thread::spawn(move || {
        let mut is_paused = false;
        loop {
            if let Some(paused) = rx_key.try_recv().ok() {
                is_paused = paused;
            };

            if !is_paused {
                if event::poll(std::time::Duration::from_millis(1)).unwrap() {
                    if let Event::Key(key) = event::read().unwrap() {
                        let key = Key::from_event(key);
                        let msg = app::MsgIn::Internal(app::InternalMsg::HandleKey(key));
                        tx_key.send(app::Task::new(0, msg, Some(key))).unwrap();
                    }
                }
            }
        }
    });

    let pipe_msg_in = app.config().pipes.msg_in.clone();
    thread::spawn(move || loop {
        let in_str = fs::read_to_string(&pipe_msg_in).unwrap_or_default();

        if !in_str.is_empty() {
            let msgs = in_str
                .lines()
                .filter_map(|s| serde_yaml::from_str::<app::ExternalMsg>(s.trim()).ok());

            msgs.for_each(|msg| {
                tx_pipe
                    .send(app::Task::new(2, app::MsgIn::External(msg), None))
                    .unwrap();
            });
            fs::write(&pipe_msg_in, "").unwrap();
        };
        thread::sleep(Duration::from_millis(10));
    });

    explorer::explore(pwd.clone(), focused_path, tx_init);

    'outer: while result.is_ok() {
        while let Some(msg) = app.pop_msg_out() {
            match msg {
                app::MsgOut::Debug(path) => {
                    fs::write(&path, serde_yaml::to_string(&app).unwrap_or_default())?;
                }

                app::MsgOut::PrintResultAndQuit => {
                    let out = if app.selected().is_empty() {
                        app.focused_node()
                            .map(|n| n.absolute_path.clone())
                            .unwrap_or_default()
                    } else {
                        app.selected()
                            .into_iter()
                            .map(|n| n.absolute_path.clone())
                            .collect::<Vec<String>>()
                            .join("\n")
                    };
                    output = Some(out);
                    break 'outer;
                }

                app::MsgOut::PrintAppStateAndQuit => {
                    let out = serde_yaml::to_string(&app)?;
                    output = Some(out);
                    break 'outer;
                }

                app::MsgOut::Refresh => {
                    if app.pwd() != &last_pwd {
                        explorer::explore(
                            app.pwd().clone(),
                            app.focused_node().map(|n| n.relative_path.clone()),
                            tx_explorer.clone(),
                        );
                        last_pwd = app.pwd().to_owned();
                    };

                    // UI
                    terminal.draw(|f| ui::draw(f, &app, &hb)).unwrap();

                    // Pipes
                    let focused = app
                        .focused_node()
                        .map(|n| n.absolute_path.clone())
                        .unwrap_or_default();

                    fs::write(&app.config().pipes.focus_out, focused).unwrap();

                    let selected = app
                        .selected()
                        .iter()
                        .map(|n| n.absolute_path.clone())
                        .collect::<Vec<String>>()
                        .join("\n");

                    fs::write(&app.config().pipes.selected_out, selected).unwrap();

                    fs::write(&app.config().pipes.mode_out, &app.mode().name).unwrap();
                }

                app::MsgOut::Call(cmd) => {
                    tx.send(true).unwrap();
                    terminal.clear()?;
                    term::disable_raw_mode().unwrap();
                    execute!(terminal.backend_mut(), term::LeaveAlternateScreen).unwrap();
                    terminal.show_cursor()?;

                    let focus_path = app
                        .focused_node()
                        .map(|n| n.absolute_path.clone())
                        .unwrap_or_default();

                    let focus_index = app
                        .directory_buffer()
                        .map(|d| d.focus)
                        .unwrap_or_default()
                        .to_string();

                    let selected = app
                        .selected()
                        .iter()
                        .map(|n| n.absolute_path.clone())
                        .collect::<Vec<String>>()
                        .join(",");

                    let directory_nodes = app
                        .directory_buffer()
                        .map(|d| {
                            d.nodes
                                .iter()
                                .map(|n| n.absolute_path.clone())
                                .collect::<Vec<String>>()
                                .join(",")
                        })
                        .unwrap_or_default();

                    let pipe_msg_in = app.config().pipes.msg_in.clone();
                    let pipe_focus_out = app.config().pipes.focus_out.clone();
                    let pipe_selected_out = app.config().pipes.selected_out.clone();

                    let app_yaml = serde_yaml::to_string(&app).unwrap_or_default();

                    let _ = std::process::Command::new(cmd.command.clone())
                        .current_dir(app.pwd())
                        .env("XPLR_FOCUS_PATH", focus_path)
                        .env("XPLR_FOCUS_INDEX", focus_index)
                        .env("XPLR_SELECTED", selected)
                        .env("XPLR_PIPE_MSG_IN", pipe_msg_in)
                        .env("XPLR_PIPE_SELECTED_OUT", pipe_selected_out)
                        .env("XPLR_PIPE_FOCUS_OUT", pipe_focus_out)
                        .env("XPLR_APP_YAML", app_yaml)
                        .env("XPLR_DIRECTORY_NODES", directory_nodes)
                        .args(cmd.args.clone())
                        .status();

                    terminal.hide_cursor()?;
                    execute!(terminal.backend_mut(), term::EnterAlternateScreen).unwrap();
                    term::enable_raw_mode().unwrap();
                    tx.send(false).unwrap();
                    terminal.draw(|f| ui::draw(f, &app, &hb)).unwrap();
                }
            };
        }

        for task in rx.try_iter() {
            app = app.enqueue(task);
        }

        let (new_app, new_result) = match app.clone().possibly_mutate() {
            Ok(a) => (a, Ok(())),
            Err(e) => (app, Err(e)),
        };

        app = new_app;
        result = new_result;

        // thread::sleep(Duration::from_millis(10));
    }

    term::disable_raw_mode().unwrap();
    execute!(terminal.backend_mut(), term::LeaveAlternateScreen).unwrap();
    terminal.show_cursor()?;

    if let Some(out) = output {
        println!("{}", out);
    }

    result
}
