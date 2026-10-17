# Maintainer: Atharva Pawar <https://github.com/Atharvapawar007>

pkgname=devchron-plus
_pkgname=devchron
_reponame=DevChron
pkgver=0.1.1
pkgrel=1
pkgdesc="Terminal Pomodoro timer and offline focus analytics suite for Hyprland/Wayland"
arch=('x86_64' 'aarch64')
url="https://github.com/Atharvapawar007/DevChron"
license=('MIT')
depends=('gcc-libs' 'glibc' 'dbus' 'hicolor-icon-theme')
makedepends=('cargo')
provides=('devchron')
conflicts=('devchron')
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('60980c727fe0e526e79e1de50ac9fdc433ec73e493852eea3c88d4e24012f620')

build() {
    cd "$_reponame-$pkgver"
    cargo build --release --locked
}

check() {
    cd "$_reponame-$pkgver"
    cargo test --locked
}

package() {
    cd "$_reponame-$pkgver"

    # Install binary
    install -Dm755 "target/release/$_pkgname" "$pkgdir/usr/bin/$_pkgname"

    # Install desktop launcher and icon
    install -dm755 "$pkgdir/usr/share/applications"
    sed "s|@BIN@|/usr/bin/$_pkgname|g" "assets/$_pkgname.desktop" > "$pkgdir/usr/share/applications/$_pkgname.desktop"
    chmod 644 "$pkgdir/usr/share/applications/$_pkgname.desktop"

    install -Dm644 "assets/$_pkgname.svg" "$pkgdir/usr/share/icons/hicolor/scalable/apps/$_pkgname.svg"
    install -Dm644 "config/default.toml" "$pkgdir/usr/share/$pkgname/default.toml"
    install -Dm755 "assets/waybar-devchron.sh" "$pkgdir/usr/share/$pkgname/scripts/waybar-devchron.sh"
    install -Dm644 "README.md" "$pkgdir/usr/share/doc/$pkgname/README.md"
    install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
