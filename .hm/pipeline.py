from __future__ import annotations

from datetime import timedelta

import harmont as hm


TRIGGERS = [hm.push(branch="main"), hm.pull_request()]
ENV = {
    "CI": "true",
    "CARGO_TERM_COLOR": "always",
    "CARGO_INCREMENTAL": "0",
}


def rust_with_system_dependencies(packages: str) -> hm.RustToolchain:
    base = hm.sh(
        f"apt-get update && apt-get install --no-install-recommends -y {packages}",
        label="Install system dependencies",
        cache=hm.ttl(timedelta(days=1)),
    )
    base = base.sh(
        "apt-get update && apt-get install --no-install-recommends -y git",
        label="Install Git",
        cache=hm.ttl(timedelta(days=1)),
    )
    return hm.rust.toolchain(base=base)


@hm.pipeline("verify", env=ENV, triggers=TRIGGERS)
def verify() -> hm.Step:
    setup = rust_with_system_dependencies(
        "curl ca-certificates build-essential pkg-config libssl-dev "
        "libasound2-dev libgtk-3-dev libwebkit2gtk-4.1-dev"
    ).installed.sh(
        "curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C /usr/local/bin && "
        "curl -fsSL https://ziglang.org/download/0.15.2/zig-x86_64-linux-0.15.2.tar.xz "
        "| tar -xJ -C /opt && ln -s /opt/zig-x86_64-linux-0.15.2/zig /usr/local/bin/zig",
        cache=hm.forever(),
    )
    return setup.sh(
        ". $HOME/.cargo/env && ./scripts/verify.sh --check",
        label="Clippy, format and tests",
    )


@hm.pipeline("web", env=ENV, triggers=TRIGGERS)
def web() -> hm.Step:
    setup = rust_with_system_dependencies(
        "curl ca-certificates build-essential pkg-config libssl-dev clang-20 llvm-20"
    ).installed.sh(
        ". $HOME/.cargo/env && rustup target add wasm32-wasip1 && "
        "cargo install wasm-bindgen-cli --version 0.2.122 --locked",
        cache=hm.on_change("Cargo.lock"),
    )
    return setup.sh(
        ". $HOME/.cargo/env && PATH=/usr/lib/llvm-20/bin:$PATH "
        "./scripts/build-block-web.sh --release",
        label="Build web bundle",
    )


@hm.pipeline("android", env=ENV, triggers=TRIGGERS)
def android() -> hm.Step:
    setup = rust_with_system_dependencies(
        "curl ca-certificates build-essential pkg-config libssl-dev openjdk-17-jdk-headless unzip"
    ).installed.sh(
        ". $HOME/.cargo/env && rustup target add aarch64-linux-android && "
        "cargo install cargo-apk --version 0.10.0 --locked && "
        "mkdir -p /opt/android-sdk/cmdline-tools && "
        "curl -fsSL https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip "
        "-o /tmp/android-command-line-tools.zip && "
        "unzip -q /tmp/android-command-line-tools.zip -d /opt/android-sdk/cmdline-tools && "
        "mv /opt/android-sdk/cmdline-tools/cmdline-tools /opt/android-sdk/cmdline-tools/latest && "
        "yes | /opt/android-sdk/cmdline-tools/latest/bin/sdkmanager --licenses >/dev/null && "
        "/opt/android-sdk/cmdline-tools/latest/bin/sdkmanager --channel=3 --install "
        "'platform-tools' 'platforms;android-35' 'build-tools;35.0.0' 'ndk;29.0.14206865'",
        cache=hm.forever(),
    )
    return setup.sh(
        ". $HOME/.cargo/env && mkdir -p target && "
        "if [ -n \"$ANDROID_DEBUG_KEYSTORE_BASE64\" ]; then "
        "printf '%s' \"$ANDROID_DEBUG_KEYSTORE_BASE64\" | base64 -d > target/android-debug.keystore; "
        "else keytool -genkeypair -v -keystore target/android-debug.keystore "
        "-storepass android -alias androiddebugkey -keypass android "
        "-dname 'CN=Android Debug,O=Android,C=US' -keyalg RSA -keysize 2048 -validity 10000; fi && "
        "sed -i 's/^package = \"com.be3.block\"$/package = \"com.be3.block.ci\"/' "
        "crates/block-app/Cargo.toml && "
        "sed -i 's/^label = \"Block\"$/label = \"Block (CI)\"/' crates/block-app/Cargo.toml && "
        "ANDROID_SDK_ROOT=/opt/android-sdk "
        "./scripts/build-block-android.sh --android-sdk /opt/android-sdk",
        label="Build Android APK",
    )


@hm.pipeline("native-linux", env=ENV, triggers=TRIGGERS)
def native_linux() -> tuple[hm.Step, ...]:
    setup = rust_with_system_dependencies(
        "curl ca-certificates build-essential pkg-config libssl-dev "
        "libasound2-dev libgtk-3-dev libwebkit2gtk-4.1-dev "
        "gcc-aarch64-linux-gnu libc6-dev-arm64-cross"
    ).installed.sh(
        ". $HOME/.cargo/env && "
        "rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu && "
        "curl -fsSL https://ziglang.org/download/0.15.2/zig-x86_64-linux-0.15.2.tar.xz "
        "| tar -xJ -C /opt && ln -s /opt/zig-x86_64-linux-0.15.2/zig /usr/local/bin/zig",
        cache=hm.forever(),
    )
    x86 = setup.fork("Linux x86_64").sh(
        ". $HOME/.cargo/env && "
        "cargo build --release --target x86_64-unknown-linux-gnu -p block-server --bins && "
        "cargo build --release --target x86_64-unknown-linux-gnu -p block-app --bins && "
        "./scripts/build-plugin-demo.sh --target x86_64-unknown-linux-gnu --profile release "
        "--output dist/linux-x86_64/plugin-package "
        "--app-executable target/x86_64-unknown-linux-gnu/release/block-app && "
        "cp target/x86_64-unknown-linux-gnu/release/block-server dist/linux-x86_64/",
        label="Build Linux x86_64",
    )
    arm = setup.fork("Linux aarch64").sh(
        ". $HOME/.cargo/env && "
        "cargo build --release --target aarch64-unknown-linux-gnu -p block-server --bins && "
        "mkdir -p dist/linux-aarch64 && "
        "cp target/aarch64-unknown-linux-gnu/release/block-server dist/linux-aarch64/",
        label="Build Linux aarch64",
        env={
            "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER": "aarch64-linux-gnu-gcc",
            "CC_aarch64_unknown_linux_gnu": "aarch64-linux-gnu-gcc",
            "AR_aarch64_unknown_linux_gnu": "aarch64-linux-gnu-ar",
        },
    )
    return x86, arm
