use std::io::prelude::*;

pub struct SshSession {
    session: ssh2::Session,
    sftp: Option<ssh2::Sftp>,
}

impl SshSession {
    pub fn new<A: std::net::ToSocketAddrs>(ip: A) -> Result<Self, ssh2::Error> {
        let tcp = match std::net::TcpStream::connect(ip) {
            Err(e) => {
                let id = match e.raw_os_error() {
                    Some(i) => i,
                    None => -1,
                };
                return Err(ssh2::Error::new(
                    ssh2::ErrorCode::Session(id),
                    "Error when creating TcpStream",
                ));
            }
            Ok(t) => t,
        };
        let mut sess = ssh2::Session::new()?;
        sess.set_tcp_stream(tcp);
        sess.handshake()?;

        let is_pi_user = {
            let env_user = std::env::var("USER");
            if env_user.is_ok() && env_user.unwrap() == "pi" {
                true
            } else {
                false
            }
        };

        if is_pi_user {
            let mut agent = sess.agent()?;

            // Connect the agent and request a list of identities
            agent.connect()?;
            // Apparently this is required...
            agent.list_identities()?;

            for identity in agent.identities()? {
                // Try to authenticate with the first identity in the agent.
                if identity.comment() == "zaczkows@omega" {
                    log::debug!("Using first identity: {}", identity.comment());
                    agent
                        .userauth("zaczkows", &identity)
                        .expect("Authentication failed");
                    break;
                }
            }
        } else {
            sess.userauth_pubkey_file(
                "zaczkows",
                Some(std::path::Path::new("/home/pi/.ssh/id_rsa.pub")),
                std::path::Path::new("/home/pi/.ssh/id_rsa"),
                Some(&std::env::var("KEY_PASSWD").expect("Missing password file for private key")),
            )
            .expect("Failed to authenticate using pubkey");
        }

        // Make sure we succeeded
        assert!(sess.authenticated());

        Ok(Self {
            session: sess,
            sftp: None,
        })
    }
    pub fn read_remote_file<F: AsRef<std::path::Path>>(
        &self,
        name: F,
    ) -> Result<Vec<u8>, ssh2::Error> {
        let path = name.as_ref();
        let (mut remote_file, _stat) = self.session.scp_recv(path)?;
        let mut contents = Vec::new();
        if remote_file.read_to_end(&mut contents).is_err() {
            return Err(ssh2::Error::new(
                ssh2::ErrorCode::Session(-1),
                "Failed to read the file from remote",
            ));
        }

        // Close the channel and wait for the whole content to be tranferred
        remote_file.send_eof()?;
        remote_file.wait_eof()?;
        remote_file.close()?;
        remote_file.wait_close()?;

        Ok(contents)
    }

    pub fn execute_remove_command(&self, cmd: &str) -> Result<String, ssh2::Error> {
        let mut channel = self.session.channel_session()?;
        channel.exec(cmd)?;
        let mut s = String::new();
        if channel.read_to_string(&mut s).is_err() {
            return Err(ssh2::Error::new(
                ssh2::ErrorCode::Session(-1),
                "Failed to read the command output content",
            ));
        }
        channel.wait_close()?;

        Ok(s)
    }

    pub fn read_remote_file_sftp<F: AsRef<std::path::Path>>(
        &mut self,
        name: F,
    ) -> Result<Vec<u8>, ssh2::Error> {
        if self.sftp.is_none() {
            self.sftp = Some(self.session.sftp()?);
        }
        let path = name.as_ref();
        let mut file = self.sftp.as_ref().unwrap().open(path).unwrap();
        let stat = file.stat().unwrap();
        log::debug!(
            "Remote file \"{}\" size: {}",
            &path.to_str().unwrap(),
            stat.size.unwrap()
        );
        let mut content: Vec<u8> = Vec::new();
        file.read_to_end(&mut content).unwrap();

        Ok(content)
    }
}
