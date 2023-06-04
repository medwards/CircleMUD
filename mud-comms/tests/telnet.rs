use std::{
    fs::{copy, read_dir},
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant},
};

use bstr::{BStr, BString};
use tempdir::TempDir;

#[test]
fn test_create_character() {
    // TODO: make random
    let port = 4001;
    let mut lib_folder = PathBuf::new();
    lib_folder.push(env!("CARGO_MANIFEST_DIR"));
    lib_folder.push("tests");
    lib_folder.push("clean_lib");
    let mut server = Server::new(port, lib_folder.as_path()).expect("Server didn't start");
    server.wait_for_start();

    // Test initial connection + welcome
    let res = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], port)));
    assert!(res.is_ok());
    let mut stream = res.unwrap();
    assert_response(
        &mut stream,
        BStr::new("\r\n                              Your MUD Name Here\r\n                              lib/text/greetings\r\n\r\n                            Based on CircleMUD 3.1,\r\n                            Created by Jeremy Elson\r\n\r\n                      A derivative of DikuMUD (GAMMA 0.0),\r\n                created by Hans-Henrik Staerfeldt, Katja Nyboe,\r\n               Tom Madsen, Michael Seifert, and Sebastian Hammer\r\n\r\nBy what name do you wish to be known? "));

    // Enter username
    stream.write("Foo\n".as_bytes()).expect("writing failed");
    assert_response(&mut stream, BStr::new("Did I get that right, Foo (Y/N)? "));

    // Confirm name
    stream.write("Y\n".as_bytes()).expect("writing failed");
    // TODO: these are the telnet echo on characters
    assert_response(
        &mut stream,
        BStr::new(b"New character.\r\nGive me a password for Foo: \xFF\xFB\x01"),
    );

    // Enter new password
    stream
        .write("password\n".as_bytes())
        .expect("writing failed");
    assert_response(&mut stream, BStr::new("\r\nPlease retype password: "));

    // confirm new password
    stream
        .write("password\n".as_bytes())
        .expect("writing succeeded");
    // TODO includes telnet echo on characters
    assert_response(
        &mut stream,
        BStr::new(b"\xFF\xFC\x01\r\nWhat is your sex (M/F)? "),
    );

    // enter sex
    stream.write("F\n".as_bytes()).expect("writing succeeded");
    assert_response(
        &mut stream,
        BStr::new("\r\nSelect a class:\r\n  [C]leric\r\n  [T]hief\r\n  [W]arrior\r\n  [M]agic-user\r\n\r\nClass: "),
    );

    // enter class
    stream.write("W\n".as_bytes()).expect("writing succeeded");
    assert_response(&mut stream, BStr::new("(lib/text/motd)\r\n\r\n      Welcome to\r\n\r\n        C    I    R    C    L    E    M    U    D         3    .    0\r\n                  \"We addict players for their own enjoyment.\"\r\n                 Created by Jeremy Elson (jelson@circlemud.org)\r\n\r\n\r\n*** PRESS RETURN: "));

    // acknowledge welcome (and enter menu)
    stream.write("\n".as_bytes()).expect("writing succeeded");
    assert_response(&mut stream, BStr::new("\r\nWelcome to CircleMUD!\r\n0) Exit from CircleMUD.\r\n1) Enter the game.\r\n2) Enter description.\r\n3) Read the background story.\r\n4) Change password.\r\n5) Delete this character.\r\n\r\n   Make your choice: "));

    // quit main menu
    stream.write("0\n".as_bytes()).expect("writing succeeded");
    assert_response(&mut stream, BStr::new("Goodbye.\r\n"));

    // verify the connection is closed
    // TODO: for some reason the connection isn't always closed yet (no, sleeping doesn't help), so try two writes
    match stream.write("\n".as_bytes()) {
        Ok(_) => {
            let err = stream
                .write("\n".as_bytes())
                .expect_err("Connection wasn't closed");

            assert_eq!(
                "Broken pipe (os error 32)", // probably different on non-UNIX
                err.to_string()
            );
        }
        Err(e) => assert_eq!("Transport endpoint is not connected", e.to_string()),
    }
}

#[test]
fn test_login_admin_character() {
    // TODO: make random
    let port = 4002;
    let mut lib_folder = PathBuf::new();
    lib_folder.push(env!("CARGO_MANIFEST_DIR"));
    lib_folder.push("tests");
    lib_folder.push("admin_lib");
    let mut server = Server::new(port, lib_folder.as_path()).expect("Server didn't start");
    server.wait_for_start();

    // Test initial connection + welcome
    let res = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], port)));
    assert!(res.is_ok());
    let mut stream = res.unwrap();
    assert_response(
        &mut stream,
        BStr::new("\r\n                              Your MUD Name Here\r\n                              lib/text/greetings\r\n\r\n                            Based on CircleMUD 3.1,\r\n                            Created by Jeremy Elson\r\n\r\n                      A derivative of DikuMUD (GAMMA 0.0),\r\n                created by Hans-Henrik Staerfeldt, Katja Nyboe,\r\n               Tom Madsen, Michael Seifert, and Sebastian Hammer\r\n\r\nBy what name do you wish to be known? "));

    // Enter username
    stream
        .write("Admin\n".as_bytes())
        .expect("writing succeeded");
    assert_response(&mut stream, BStr::new(b"Password: \xFF\xFB\x01"));

    // enter password
    stream
        .write("password\n".as_bytes())
        .expect("writing succeeded");
    assert_response(&mut stream, BStr::new(b"\xFF\xFC\x01\r\n(lib/text/imotd)\r\n\r\nWelcome to the long-awaited, oft-belated, highly-rated CircleMUD 3.1!\r\n\r\nThis is the immortal MOTD -- the file that immortals will see when they\r\nlog in to the game.  You should change it to something more interesting\r\nwhen you get a chance (as well as most of the other files in lib/text.)\r\n\r\nIf you need help with CircleMUD, please write to help@circlemud.org\r\n\r\nIf you would like to report a bug, please write to bugs@circlemud.org\r\n\r\nFor all other general discussion, you might want to join the CircleMUD\r\nMailing List.  If you wish to subscribe to the mailing list, send mail\r\nto <listserv@post.queensu.ca> with:\r\n   subscribe circle <first name> <last name>\r\nin the body of the message.\r\n\r\n\r\n*** PRESS RETURN: "));

    // acknowledge immortal motd
    stream.write("\n".as_bytes()).expect("writing succeeded");
    assert_response(&mut stream, BStr::new("\r\nWelcome to CircleMUD!\r\n0) Exit from CircleMUD.\r\n1) Enter the game.\r\n2) Enter description.\r\n3) Read the background story.\r\n4) Change password.\r\n5) Delete this character.\r\n\r\n   Make your choice: "));

    // quit main menu
    stream.write("0\n".as_bytes()).expect("writing succeeded");
    assert_response(&mut stream, BStr::new("Goodbye.\r\n"));

    // verify the connection is closed
    // TODO: for some reason the connection isn't always closed yet (no, sleeping doesn't help), so try two writes
    match stream.write("\n".as_bytes()) {
        Ok(_) => {
            let err = stream
                .write("\n".as_bytes())
                .expect_err("Connection wasn't closed");

            assert_eq!(
                "Broken pipe (os error 32)", // probably different on non-UNIX
                err.to_string()
            );
        }
        Err(e) => assert_eq!("Transport endpoint is not connected", e.to_string()),
    }
}

fn assert_response(stream: &mut TcpStream, expected: &BStr) {
    let length = expected.len();
    let mut buffer = BString::new(Vec::new());
    buffer.resize(length, 0);
    stream.read(buffer.as_mut_slice()).expect("successful read");
    assert_eq!(expected, buffer);
}

struct Server {
    child: Child,
    files: TempDir,
}

impl Server {
    fn new(port: u16, source: &Path) -> std::io::Result<Self> {
        let dir = setup_mud_files(source)?;

        let server = start_server(dir.path(), port)?;

        Ok(Server {
            child: server,
            files: dir,
        })
    }

    fn wait_for_start(&mut self) {
        let out = self
            .child
            .stderr
            .as_mut()
            .expect("server started and stdout available");

        let started = Instant::now();
        let mut reader = BufReader::new(out).lines();
        loop {
            let next = reader.next();
            if let Some(ref line) = next {
                if line
                    .as_ref()
                    .map(|ref r| r.contains("Entering game loop."))
                    .unwrap_or(false)
                {
                    return;
                }
            }
            if started.elapsed() > Duration::from_secs(30) {
                panic!("server didn't finish starting");
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.child.kill().expect("Child process wasn't killed");
    }
}

fn start_server(lib: &Path, port: u16) -> std::io::Result<Child> {
    let mut lib_folder = PathBuf::new();
    lib_folder.push(env!("CARGO_MANIFEST_DIR"));
    lib_folder.push("..");
    lib_folder.push("bin");
    lib_folder.push("circle");
    //Command::new("/tmp/CircleMUD/bin/circle")
    Command::new(lib_folder.as_os_str())
        .args([
            "-d",
            lib.to_str().expect("lib dir not present"),
            port.to_string().as_str(),
        ])
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
}

fn setup_mud_files(source: &Path) -> std::io::Result<TempDir> {
    let dir = TempDir::new("test_mud_files")?;

    copy_recursively(source, dir.as_ref())?;

    Ok(dir)
}

/// Copy files from source to destination recursively.
pub fn copy_recursively(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> std::io::Result<()> {
    std::fs::create_dir_all(&destination)?;
    for entry in read_dir(source)? {
        let entry = entry?;
        let filetype = entry.file_type()?;
        if filetype.is_dir() {
            copy_recursively(entry.path(), destination.as_ref().join(entry.file_name()))?;
        } else {
            copy(entry.path(), destination.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
