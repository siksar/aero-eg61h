# aero_eg61h — ağaç-dışı çekirdek modülü
#
# NixOS'ta doğrudan `make` çalışmaz: /lib/modules/$(uname -r)/build yok ve
# CachyOS-lto çekirdeği clang/LLVM=1 ile derlendi (gcc, kernel Makefile'ının
# verdiği -mstack-alignment=8 / -fsplit-lto-unit gibi bayrakları tanımıyor).
# Doğru araç zincirini flake'ten çeken sarmalayıcı: ./build.sh

obj-m += aero-eg61h.o

aero-eg61h-y := aero-main.o aero-hwmon.o

KDIR ?= /lib/modules/$(shell uname -r)/build
PWD  := $(CURDIR)

all:
	$(MAKE) -C $(KDIR) M=$(PWD) modules

clean:
	$(MAKE) -C $(KDIR) M=$(PWD) clean

.PHONY: all clean
