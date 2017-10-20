movl $0x87654321, %eax

pushl %ebp
movl %esp, %ebp

// single argument is pointer to thunk
pushl %eax
call *(%eax)
addl $4, %esp
// returns the nvapi pointer in eax

popl %ebp

// tail call
jmp *%eax
