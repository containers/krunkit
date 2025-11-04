#!/bin/sh

if [ -z $HOMEBREW_PREFIX ]; then
	echo "Can't find Homebrew prefix"
	exit -1
fi

RPATH=/opt/podman/lib
VERSION=`grep ^version Cargo.toml | sed -E 's/version = "(.*)"/\1/'`

mkdir -p release
mkdir -p release/bin
mkdir -p release/lib

BIN=release/bin
LIB=release/lib

cp $HOMEBREW_PREFIX/bin/krunkit $BIN
install_name_tool -change /opt/homebrew/opt/libkrun-efi/lib/libkrun-efi.1.dylib @rpath/libkrun-efi.dylib $BIN/krunkit
install_name_tool -add_rpath $RPATH $BIN/krunkit
codesign --remove-signature $BIN/krunkit

cp $HOMEBREW_PREFIX/lib/libkrun-efi.dylib $LIB
install_name_tool -id @rpath/libkrun-efi.dylib $LIB/libkrun-efi.dylib
install_name_tool -change /opt/homebrew/opt/libepoxy/lib/libepoxy.0.dylib @rpath/libepoxy.0.dylib $LIB/libkrun-efi.dylib
install_name_tool -change /opt/homebrew/opt/virglrenderer/lib/libvirglrenderer.1.dylib @rpath/libvirglrenderer.1.dylib $LIB/libkrun-efi.dylib
codesign --remove-signature $LIB/libkrun-efi.dylib

cp $HOMEBREW_PREFIX/lib/libepoxy.0.dylib $LIB
install_name_tool -id @rpath/libepoxy.0.dylib $LIB/libepoxy.0.dylib
codesign --remove-signature $LIB/libepoxy.0.dylib

cp $HOMEBREW_PREFIX/lib/libvirglrenderer.1.dylib $LIB
install_name_tool -id @rpath/libvirglrenderer.1.dylib $LIB/libvirglrenderer.1.dylib
install_name_tool -change /opt/homebrew/opt/molten-vk/lib/libMoltenVK.dylib @rpath/libMoltenVK.dylib $LIB/libvirglrenderer.1.dylib
install_name_tool -change /opt/homebrew/opt/libepoxy/lib/libepoxy.0.dylib @rpath/libepoxy.0.dylib $LIB/libvirglrenderer.1.dylib
codesign --remove-signature $LIB/libvirglrenderer.1.dylib

cp $HOMEBREW_PREFIX/lib/libMoltenVK.dylib $LIB
install_name_tool -id @rpath/libMoltenVK.dylib $LIB/libMoltenVK.dylib
codesign --remove-signature $LIB/libMoltenVK.dylib

# Check there aren't any references to the Homebrew prefix in the binaries
for i in $BIN/krunkit $LIB/libkrun-efi.dylib $LIB/libepoxy.0.dylib $LIB/libvirglrenderer.1.dylib $LIB/libMoltenVK.dylib; do
	otool -L $i | grep $HOMEBREW_PREFIX
	if [ $? == 0 ]; then
		echo "ERROR: $i still has references to HOMEBREW"
		exit -1
	fi
done

cd release
tar czf ../krunkit-podman-unsigned-$VERSION.tgz *

