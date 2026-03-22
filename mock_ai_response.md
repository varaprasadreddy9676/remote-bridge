I have updated the configuration and the main application logic.

File: src/config.rs
```rust
pub struct Config {
    pub port: u16,
}
```

Now, please install the dependencies and restart the service:

```bash
cargo build
sudo systemctl restart my-app
```
