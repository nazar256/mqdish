#[cfg(test)]
mod config_test {
    use std::fs::{File, remove_file};
    use std::io::Write;
    use crate::shared::config::*;
    use rand::{distributions::Alphanumeric, Rng};

    struct TempConfigFile {
        path: String,
    }

    impl TempConfigFile {
        fn new(contents: String) -> Self {
            let random_name: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(10)
                .map(char::from)
                .collect();
            let file_name = format!("./config_{}.yaml", random_name);

            let mut temp_file = File::create(&file_name).unwrap();
            temp_file.write_all(contents.as_bytes()).unwrap();

            TempConfigFile { path: file_name }
        }
    }

    impl Drop for TempConfigFile {
        fn drop(&mut self) {
            remove_file(&self.path).expect("Failed to delete temp file");
        }
    }

    #[test]
    fn test_login_pass_amqp() {
        let file_contents = r#"
            connection: "amqp://"
            credentials:
                login: "user"
                password: "pass"
            bus_params:
                type: AMQP
                params:
                  vhost: "/"
                "#
            .to_string();

        let temp = TempConfigFile::new(file_contents);

        // Load the configuration
        let config = AppConfig::load(Some(temp.path.clone())).unwrap();

        // Test the loaded configuration
        match config.credentials {
            Credentials::LoginPassword(login_password) => {
                assert_eq!(login_password.login, "user");
                assert_eq!(login_password.password, "pass");
            }
            _ => panic!("Incorrect credentials type"),
        }

        match config.bus_params {
            BusParams::AMQP(AMQPParams{ .. }) => (),
        }
    }
}