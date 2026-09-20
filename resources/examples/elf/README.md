# Building and testing the examples

## Install the GCC toolchain for your platform

```sh
sudo apt install build-essential            # For Ubuntu/Debian
sudo dnf groupinstall "Development Tools"   # For Fedora
sudo pacman -S base-devel                   # For Arch Linux
```

## Install the cross-compilation toolchain

```sh
sudo apt install gcc-x86-64-linux-gnu gcc-aarch64-linux-gnu gcc-riscv64-linux-gnu gcc-s390x-linux-gnu gcc-powerpc64le-linux-gnu gcc-loongarch64-linux-gnu
```

Note that the package names may vary depending on your Linux distribution, for example, on Arch Linux, the `Aarch64` toolchain is called `aarch64-linux-gnu-gcc`.

And you should exclude the specific platform package that you are currently using, for example, if you are on an `x86_64` machine, you can skip installing `gcc-x86-64-linux-gnu`.

## Install QEMU user-mode emulators

```sh
sudo apt install qemu-user
```

## Build and test the examples

```sh
cd asm
./build.sh all
./test.sh all
cd ..

cd gcc
./build.sh all
./test.sh all
cd ..
```
