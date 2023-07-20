pkgname=kcshot
pkgver=0.1.0
pkgrel=1
pkgdesc='Screenshot utility for Linux'
url="https://github.com/RealKC/$pkgname"
arch=('x86_64')
license=('custom:EUPL-1.2')
depends=('gtk4' 'sqlite' 'xdg-utils')
makedepends=('cargo' 'glib2' 'meson' 'blueprint-compiler')
optdepends=('xdg-desktop-portal: Wayland support')
source=("git+https://github.com/RealKC/$pkgname")
sha256sums=(SKIP)

build() {
    export RUSTUP_TOOLCHAIN=stable
    arch-meson "$pkgname" build
    meson compile -C build
}

package() {
    meson install -C build --destdir "$pkgdir"
}
