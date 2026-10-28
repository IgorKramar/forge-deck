use std::{fs, process::Command};

#[test]
fn cli_saved_language_and_locale_have_explicit_priority() {
    let directory = std::env::temp_dir().join(format!("deck-cli-language-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let run = |arguments: &[&str], locale: &str| {
        Command::new(env!("CARGO_BIN_EXE_gitlab-deck"))
            .args(arguments)
            .env("XDG_CONFIG_HOME", &directory)
            .env("LC_ALL", "")
            .env("LC_MESSAGES", locale)
            .env("LANG", "ru_RU.UTF-8")
            .output()
            .unwrap()
    };
    let check = ["--demo", "--check"];
    let all = Command::new(env!("CARGO_BIN_EXE_gitlab-deck"))
        .args(check)
        .env("XDG_CONFIG_HOME", &directory)
        .env("LC_ALL", "C.UTF-8")
        .env("LC_MESSAGES", "ru_RU.UTF-8")
        .env("LANG", "ru_RU.UTF-8")
        .output()
        .unwrap();
    assert!(
        String::from_utf8(all.stdout)
            .unwrap()
            .contains("Connection verified")
    );
    assert!(
        String::from_utf8(run(&check, "").stdout)
            .unwrap()
            .contains("Соединение проверено")
    );
    assert!(
        String::from_utf8(run(&check, "en_US.UTF-8").stdout)
            .unwrap()
            .contains("Connection verified")
    );
    let config = directory.join("gitlab-deck/config.json");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    let before =
        r#"{"host":"gitlab.example.org","language":"en","favorites":{"gitlab.example.org":[101]}}"#;
    fs::write(&config, before).unwrap();
    assert!(
        String::from_utf8(run(&check, "").stdout)
            .unwrap()
            .contains("Connection verified")
    );
    assert!(
        String::from_utf8(run(&["--demo", "--check", "--lang", "ru"], "en_US.UTF-8").stdout)
            .unwrap()
            .contains("Соединение проверено")
    );
    assert_eq!(fs::read_to_string(&config).unwrap(), before);
    assert_eq!(run(&["--lang", "xx"], "").status.code(), Some(2));
    assert!(
        String::from_utf8(run(&["--help"], "").stdout)
            .unwrap()
            .contains("--lang")
    );
    let new_config = directory.join("forge-deck/config.json");
    fs::create_dir_all(new_config.parent().unwrap()).unwrap();
    fs::write(
        &new_config,
        r#"{"provider":"github","language":"en","hosts":{"github":"github.example.org"}}"#,
    )
    .unwrap();
    let output = run(&check, "");
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("3 projects")
    );
    let output = run(&["--demo", "--check", "--provider", "gitlab"], "");
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("7 projects")
    );
    assert_eq!(run(&["--provider", "bad"], "").status.code(), Some(2));
    fs::remove_dir_all(directory).unwrap();
}
