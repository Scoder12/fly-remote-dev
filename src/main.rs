use color_eyre::eyre::{eyre, WrapErr};
use procfs::net::TcpState;
use std::{
    convert::TryInto,
    env,
    fs::{create_dir_all, write},
    path::PathBuf,
    process::{Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};

const SSH_PORT: u16 = 22;
const CODE_SERVER_PORT: u16 = 8080;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let home: PathBuf = env::var_os("HOME")
        .ok_or(eyre!("HOME var not set"))?
        .try_into()
        .wrap_err("$HOME is invalid")?;
    let home = home
        .canonicalize()
        .wrap_err("Unable to canonicalize $HOME")?;
    let xdg_config_home: PathBuf = env::var_os("XDG_CONFIG_HOME")
        .map(|p| p.into())
        .unwrap_or_else(|| home.clone().join(".config"));
    let xdg_data_home: PathBuf = env::var_os("XDG_DATA_HOME")
        .map(|p| p.into())
        .unwrap_or_else(|| home.clone().join(".local").join("share"));

    let ssh_pubkey = env::var("SSH_PUBKEY").wrap_err("SSH_PUBKEY var invalid")?;
    let auth_keys_file = home.join(".ssh").join("authorized_keys");
    create_dir_all(auth_keys_file.parent().unwrap())?;
    write(auth_keys_file, ssh_pubkey)?;

    let code_server_password =
        env::var("CODE_SERVER_PASS").wrap_err("CODE_SERVER_PASS var invalid")?;
    let code_server_config_path: PathBuf = xdg_config_home.join("code-server").join("config.yaml");
    create_dir_all(code_server_config_path.parent().unwrap())?;
    write(
        code_server_config_path,
        format!(
            "bind-addr: 0.0.0.0:{}\n\
            auth: password\n\
            password: {}\n\
            cert: false\n",
            CODE_SERVER_PORT, code_server_password
        ),
    )?;

    let settings_json = include_bytes!("settings.json");
    let settings_json_path = xdg_data_home.join("code-server");
    create_dir_all(settings_json_path.parent().unwrap())?;
    write(settings_json_path, settings_json)?;

    let status = Command::new("sudo")
        .arg("service")
        .arg("ssh")
        .arg("start")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    assert!(status.success());

    Command::new("code-server")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut last_activity = Instant::now();

    loop {
        if count_conns()? > 0 {
            last_activity = Instant::now();
        } else {
            let idle_time = last_activity.elapsed();
            println!("Idle for {idle_time:?}");
            if idle_time > Duration::from_secs(60) {
                println!("Stopping machine. Goodbye!");
                std::process::exit(0)
            }
        }
        sleep(Duration::from_secs(5));
    }
}

fn count_conns() -> color_eyre::Result<usize> {
    Ok(procfs::net::tcp()?
        .into_iter()
        // don't count listen, only established
        .filter(|entry| matches!(entry.state, TcpState::Established))
        .filter(|entry| matches!(entry.local_address.port(), SSH_PORT | CODE_SERVER_PORT))
        .count())
}
