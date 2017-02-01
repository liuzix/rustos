global mp_start
global mp_end
global temp_gdt_pointer

extern check_cpuid
extern check_long_mode
extern enable_paging
extern set_up_SSE
extern gdt_pointer
extern gdt_data
extern gdt_code
extern mp_main
extern create_stack

%define get_addr(a) (a - mp_start + 0x1000)

section .mp
bits 16
mp_start:
    cli

    ; set all segment selector to zero. so we can use phys address
	xor ax, ax
    mov ss, ax
    mov ds, ax
    mov es, ax
    lgdt [get_addr(temp_gdt_pointer)]
    ; enable protected mode
    mov eax, cr0
    or eax, 1
    mov cr0, eax

    jmp 8: get_addr(mp_start_32)


bits 32

mp_start_32:

    mov eax, 0x10
    mov ds, eax
    mov es, eax
    mov ss, eax

    mov eax, 0x0
    mov fs, eax
    mov gs, eax
    jmp 0x8: acquire_lock
.locked:
    mov esp, ap_temp_stack

    mov eax, check_cpuid
    call eax

    mov eax, check_long_mode
    call eax

    mov eax, enable_paging
    call eax

    mov eax, set_up_SSE
    call eax

    mov eax, gdt_pointer

    lgdt [eax]
    mov ax, gdt_data
    mov ss, ax
    mov ds, ax
    mov es, ax
    jmp gdt_code: get_addr(.have_long_mode)


bits 64
align 8
.have_long_mode:
    mov rax, create_stack
    call rax
    mov rsp, rax
    sub rsp, 8
    ;and rsp, -16

    mov rbp, rsp
    mfence
    mov word [v_lock], 0
    ;
    mov rax, mp_main
    jmp rax


bits 32
align 8
temp_gdt:
    dq 0

    dw 0xffff, 0x0000
    db 0, 10011010b, 11001111b, 0

    dw 0xffff, 0x0000
    db 0, 10010010b, 11001111b, 0

align 8
temp_gdt_pointer:
    dw 23
    dd get_addr(temp_gdt)
    dq 0



mp_end:
    nop

align 4096
resb 4096
ap_temp_stack:


v_lock:
    dw 0

acquire_lock:
    mov ax, 1
    xchg ax, [v_lock]
    test ax, ax
    je .got_lock_1 ; loop if lock held
    pause
    jmp acquire_lock
.got_lock_1:
    mfence
    mov eax, get_addr(mp_start_32.locked)
    jmp eax
