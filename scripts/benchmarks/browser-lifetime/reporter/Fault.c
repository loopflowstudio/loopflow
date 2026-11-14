// A hardware memory fault, rather than raise(SIGSEGV), exercises Mach handling.
void diagnostic_fault(void) {
    *(volatile unsigned char *)1 = 1;
}
