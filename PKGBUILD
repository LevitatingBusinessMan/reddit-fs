#Maintainer: Rein Fernhout (LevitatingBusinessMan) <me@levitati.ng>

pkgname=reddit-fs
pkgver=r35.18ab93a
arch=("x86_64")
pkgrel=1
pkgdesc="Reddit as a filesystem"
license=("MIT")
makedepends=("cargo")
url="https://github.com/LevitatingBusinessMan/$pkgname"
source=("git+https://github.com/LevitatingBusinessMan/$pkgname.git")
sha256sums=("SKIP")

pkgver() {
	cd "$pkgname"
	printf "r%s.%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

build() {
	cd "${srcdir}/${pkgname}"
	cargo build --release
}

package() {
	cd "${srcdir}/${pkgname}"

	install -Dm755 target/release/reddit-fs $pkgdir/usr/bin/reddit-fs
}
