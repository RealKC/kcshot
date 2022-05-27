pkgname=kcshot
pkgver=0.1.0
pkgrel=1
pkgdesc='Screenshot utility for Linux'
url="https://github.com/RealKC/$pkgname"
arch=('x86_64')
license=('custom:EUPL-1.2')
depends=('gtk4' 'sqlite' 'xdg-utils')
makedepends=('cargo' 'glib2')
optdepends=('xdg-desktop-portal: Wayland support')
source=("git+https://github.com/RealKC/$pkgname")
sha256sums=(SKIP)

prepare() {
    cd "$srcdir/$pkgname"

    cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "$srcdir/$pkgname"
    
    export RUSTUP_TOOLCHAIN=stable
    cargo build --frozen --features xdg-paths --release --target-dir target
}

package() {
    cd "$srcdir/$pkgname"

    install -Dm755 target/release/kcshot-rs "$pkgdir/usr/bin/kcshot"

    install -Dm644 resources/logo/kcshot_logo_dark.svg "$pkgdir/usr/share/icons/hicolor/scalable/kcshot.svg"
    install -Dm644 resources/kc.kcshot.gschema.xml "$pkgdir/usr/share/glib-2.0/schemas/kc.kcshot.gschema.xml"

    install -Dm644 LICENSE.txt "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
