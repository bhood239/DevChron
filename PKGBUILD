# Maintainer: Atharva Pawar <https://github.com/Atharvapawar007>
pkgname=devchron
pkgver=0.1.1
pkgrel=1
pkgdesc="A stunning TUI Pomodoro timer and offline focus analytics suite for Hyprland/Wayland"
arch=('x86_64')
url="https://github.com/Atharvapawar007/DevChron"
license=('MIT')
depends=('dbus')
makedepends=('rust' 'cargo')
optdepends=(
    'waybar: Status bar integration'
    'jq: JSON parsing for status bar scripts'
)
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
    cd "DevChron-$pkgver"
    export CARGO_TARGET_DIR=target
    cargo build --release --locked
}

check() {
    cd "DevChron-$pkgver"
    cargo test --release --locked
}

package() {
    cd "DevChron-$pkgver"
    
    # Install binary
    install -Dm755 "target/release/$pkgname" \
        "$pkgdir/usr/bin/$pkgname"
    
    # Install default config
    install -Dm644 "config/default.toml" \
        "$pkgdir/usr/share/$pkgname/default.toml"
    
    # Install documentation
    install -Dm644 README.md \
        "$pkgdir/usr/share/doc/$pkgname/README.md"
    
    # Install license
    install -Dm644 LICENSE \
        "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
