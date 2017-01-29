#!/bin/bash
mkdir -p isofiles/boot/grub
cp build/kernel-x86_64.bin isofiles/boot/kernel.bin
cp grub.cfg isofiles/boot/grub
grub-mkrescue -o myos.iso isofiles 2> /dev/null
rm -r isofiles
scp myos.iso zixiong@192.168.2.127:~/myos.iso
