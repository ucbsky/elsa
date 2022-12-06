

#include <cstdio>
#include <iostream>
#include <unistd.h>
#include "demo.h"
// extern "C" void print_hello_message(){
//     std::cout << "HELLO from C" << std::endl;
// }

// extern "C" void call_hello_n_times(unsigned i) 
// {
//     for (unsigned k = 0; k < i; k++) {
//         std::cout << "HELLO from CPP: "<< i << std::endl;
//         sleep(1);
//     }
// }

// extern "C" int gen_rand_int() {
//     return make_random_int();
// }

// extern "C" void run_ot(int party, int port, int length) {
//     return run_test(party, port, length);
// }

extern "C" unsigned long long emp_rot(int party, const char* remote_addr, int port, long long int count, int mode, unsigned char* data0, unsigned char* data1) {
    return random_ot(party, remote_addr, port, count, mode, data0, data1);
}

//extern "C" void playground(){
//    playground_func();
//}