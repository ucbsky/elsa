#ifndef C6AB717B_F2B3_480E_BCB5_3CCC22694810
#define C6AB717B_F2B3_480E_BCB5_3CCC22694810
#include <cstdint>

int make_random_int();
void run_test(int party, int port, int length);
uint64_t random_ot(int party, const char* remote_addr, int port, long long int count, int mode, unsigned char* data0, unsigned char* data1);
// TODO; remove this after release
void playground_func();

#endif /* C6AB717B_F2B3_480E_BCB5_3CCC22694810 */
