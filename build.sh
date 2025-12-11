!/bin/sh

cargo build --release
cd target/release
doas mv perano-lang /bin/perano
doas chmod +x /bin/perano