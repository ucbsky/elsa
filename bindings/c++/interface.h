
#ifndef D578F7EE_F4E9_4065_A0B5_5C4556CCA809
#define D578F7EE_F4E9_4065_A0B5_5C4556CCA809

// void print_hello_message(); // not needed for now

// void call_hello_n_times(unsigned i);  // not needed for now

// int gen_rand_int(); // not needed for now

// void run_ot(int party, int port, int length); // not needed for now

/**
 if party == 1 (ALICE), we are hosting, so `remote_addr` can be nullptr. Otherwise (BOB), `remote_addr` must be non-nullptr.
 Return number of bytes sent.
 **/
unsigned long long emp_rot(int party, const char* remote_addr, int port, long long int count, int mode, unsigned char* data0, unsigned char* data1);

//// just some playground function
//void playground();

#endif /* D578F7EE_F4E9_4065_A0B5_5C4556CCA809 */
