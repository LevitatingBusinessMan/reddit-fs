# reddit-fs
Accessing reddit through a filesystem
```SH
$ reddit-fs ~/reddit &
$ ls ~/reddit/linux
'Announcing Fedora Linux 37'
'Four years of SourceHut (SourceHut is a open source github alternative)'
'Free CLI util for computer algebra - to evaluate, solve equations, differentiate, and more!'
'How to Setup Encrypted Chat on Librem Devices'
'I collated some scripts that allow you to have Wifi in Debian initramfs'
'Join us at UbuCon Asia in Seoul this November!'
'Mold linker may not switch to a source-available license'
'NEW Ubuntu Linux images on Intel processors'
'[OC] jfchmotfsdynfetch - The MOST minimal fetch tool that fetches precisely NO information about your PC'
'Osboot is now part of Libreboot (new release soon!)'
'Ranger file manager over ssh'
'Sapling: Source control that’s user-friendly and scalable'
'This is the script I use to mount multiple network locations through SSH'
'Unity 7.6 is now available for Arch Linux'
'Unreal Engine 5.1.0 binary alongside Quixel Bridge Plugin released'
'What is the state of Wayland?'
'Windows Powershell on Linux?'
'Windows Subsystem for Linux (WSL) v1.0.0 released'
$ cat ~/reddit/linux/Announcing\ Fedora\ Linux\ 37
https://fedoramagazine.org/announcing-fedora-37/
```

#### Installation
Make sure `fuse` is installed.
```SH
cargo install reddit-fs
```
