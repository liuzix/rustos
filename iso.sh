#!/bin/bash
echo 'copying files'
mkdir -p isofiles/boot/grub
cp build/kernel-x86_64.bin isofiles/boot/kernel.bin
cp grub.cfg isofiles/boot/grub
echo 'building image'
grub-mkrescue -o myos.iso isofiles 
echo 'cleaning up'
#rm -r isofiles
#scp myos.iso zixiong@192.168.2.127:~/myos.iso
