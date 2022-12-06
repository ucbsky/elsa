#include "demo.h"

#include <cstdint>
#include <emmintrin.h>
#include <emp-tool/emp-tool.h>
#include "emp-ot/emp-ot.h"
#include <emp-tool/utils/block.h>
#include <emp-tool/utils/f2k.h>
#include <iostream>

using namespace std;

int threads = 1;

//int make_random_int()
//{
//    emp::PRG prg;
//    int rand_int;
//    prg.random_data(&rand_int, sizeof(rand_int));
//    return rand_int;
//}
//
//template <typename T>
//double test_ot(T * ot, NetIO *io, int party, int64_t length) {
//	block *b0 = new block[length], *b1 = new block[length],
//	*r = new block[length];
//	PRG prg(fix_key);
//	prg.random_block(b0, length);
//	prg.random_block(b1, length);
//	bool *b = new bool[length];
//	PRG prg2;
//	prg2.random_bool(b, length);
//
//	auto start = clock_start();
//	if (party == ALICE) {
//		ot->send(b0, b1, length);
//	} else {
//		ot->recv(r, b, length);
//	}
//	io->flush();
//	long long t = time_from(start);
//	if (party == BOB) {
//		for (int64_t i = 0; i < length; ++i) {
//			if (b[i]){ if(!cmpBlock(&r[i], &b1[i], 1)) {
//				std::cout <<i<<"\n";
//				error("wrong!\n");
//			}}
//			else { if(!cmpBlock(&r[i], &b0[i], 1)) {
//				std::cout <<i<<"\n";
//				error("wrong!\n");
//			}}
//		}
//	}
//    std::cout << "Tests passed.\t";
//	delete[] b0;
//	delete[] b1;
//	delete[] r;
//	delete[] b;
//	return t;
//}
//
//template <typename T>
//double test_cot(T * ot, NetIO *io, int party, int64_t length) {
//	block *b0 = new block[length], *r = new block[length];
//	bool *b = new bool[length];
//	block delta;
//	PRG prg;
//	prg.random_block(&delta, 1);
//	prg.random_bool(b, length);
//
//	io->sync();
//	auto start = clock_start();
//	if (party == ALICE) {
//		ot->send_cot(b0, length);
//		delta = ot->Delta;
//	} else {
//		ot->recv_cot(r, b, length);
//	}
//	io->flush();
//	long long t = time_from(start);
//	if (party == ALICE) {
//		io->send_block(&delta, 1);
//		io->send_block(b0, length);
//	}
//	else if (party == BOB) {
//		io->recv_block(&delta, 1);
//		io->recv_block(b0, length);
//		for (int64_t i = 0; i < length; ++i) {
//			block b1 = b0[i] ^ delta;
//			if (b[i]) {
//				if (!cmpBlock(&r[i], &b1, 1))
//					error("COT failed!");
//			} else {
//				if (!cmpBlock(&r[i], &b0[i], 1))
//					error("COT failed!");
//			}
//		}
//	}
//	std::cout << "Tests passed.\t";
//	io->flush();
//	delete[] b0;
//	delete[] r;
//	delete[] b;
//	return t;
//}
//
//template <typename T>
//double test_rot(T* ot, NetIO *io, int party, int64_t length) {
//	block *b0 = new block[length], *r = new block[length];
//	block *b1 = new block[length];
//	bool *b = new bool[length];
//	PRG prg;
//	prg.random_bool(b, length);
//
//	io->sync();
//	auto start = clock_start();
//	if (party == ALICE) {
//		ot->send_rot(b0, b1, length);
//	} else {
//		ot->recv_rot(r, b, length);
//	}
//	io->flush();
//	long long t = time_from(start);
//	if (party == ALICE) {
//		io->send_block(b0, length);
//		io->send_block(b1, length);
//	} else if (party == BOB) {
//		io->recv_block(b0, length);
//		io->recv_block(b1, length);
//		for (int64_t i = 0; i < length; ++i) {
//			if (b[i])
//				assert(cmpBlock(&r[i], &b1[i], 1));
//			else
//				assert(cmpBlock(&r[i], &b0[i], 1));
//		}
//	}
//	std::cout << "Tests passed.\t";
//	io->flush();
//	delete[] b0;
//	delete[] b1;
//	delete[] r;
//	delete[] b;
//	return t;
//}

//void run_test(int party, int port, int length) {
//	cout << "Inside EMP's OT test" << endl;
//	cout << "Setting up connection..." << endl;
//	NetIO * io = new NetIO(party==ALICE ? nullptr:"127.0.0.1", port);
//	OTNP<NetIO> * np = new OTNP<NetIO>(io);
//	cout <<"128 NPOTs:\t"<<test_ot<OTNP<NetIO>>(np, io, party, 128)<<" us"<<endl;
//	delete np;
//	IKNP<NetIO> * iknp = new IKNP<NetIO>(io);
//	cout <<"Passive IKNP OT\t"<<double(length)/test_ot<IKNP<NetIO>>(iknp, io, party, length)*1e6<<" OTps"<<endl;
//	cout <<"Passive IKNP COT\t"<<double(length)/test_cot<IKNP<NetIO>>(iknp, io, party, length)*1e6<<" OTps"<<endl;
//	cout <<"Passive IKNP ROT\t"<<double(length)/test_rot<IKNP<NetIO>>(iknp, io, party, length)*1e6<<" OTps"<<endl;
//	delete iknp;
//
//	OTCO<NetIO> * co = new OTCO<NetIO>(io);
//	cout <<"128 COOTs:\t"<<test_ot<OTCO<NetIO>>(co, io, party, 128)<<" us"<<endl;
//	delete co;
//	iknp = new IKNP<NetIO>(io, true);
//	cout <<"Active IKNP OT\t"<<double(length)/test_ot<IKNP<NetIO>>(iknp, io, party, length)*1e6<<" OTps"<<endl;
//	cout <<"Active IKNP COT\t"<<double(length)/test_cot<IKNP<NetIO>>(iknp, io, party, length)*1e6<<" OTps"<<endl;
//	cout <<"Active IKNP ROT\t"<<double(length)/test_rot<IKNP<NetIO>>(iknp, io, party, length)*1e6<<" OTps"<<endl;
//	delete iknp;
//
//	FerretCOT<NetIO> * ferretcot = new FerretCOT<NetIO>(party, threads, &io, false);
//	cout <<"Passive FERRET OT\t"<<double(length)/test_ot<FerretCOT<NetIO>>(ferretcot, io, party, length)*1e6<<" OTps"<<endl;
//	cout <<"Passive FERRET COT\t"<<double(length)/test_cot<FerretCOT<NetIO>>(ferretcot, io, party, length)*1e6<<" OTps"<<endl;
//	cout <<"Passive FERRET ROT\t"<<double(length)/test_rot<FerretCOT<NetIO>>(ferretcot, io, party, length)*1e6<<" OTps"<<endl;
//	delete ferretcot;
//	ferretcot = new FerretCOT<NetIO>(party, threads, &io, true);
//	cout <<"Active FERRET OT\t"<<double(length)/test_ot<FerretCOT<NetIO>>(ferretcot, io, party, length)*1e6<<" OTps"<<endl;
//	cout <<"Active FERRET COT\t"<<double(length)/test_cot<FerretCOT<NetIO>>(ferretcot, io, party, length)*1e6<<" OTps"<<endl;
//	cout <<"Active FERRET ROT\t"<<double(length)/test_rot<FerretCOT<NetIO>>(ferretcot, io, party, length)*1e6<<" OTps"<<endl;
//	delete ferretcot;
//
//
//	delete io;
//}

template <typename T>
double rot(T* ot, NetIO *io, int party, int64_t length, unsigned char* m0, unsigned char* m1, unsigned char* choice) {
	block *b0 = new block[length], *r = new block[length];
	block *b1 = new block[length];
	bool *b = new bool[length];
	PRG prg;
	prg.random_bool(b, length);

	io->sync();
	auto start = clock_start();
	if (party == ALICE) {
		ot->send_rot(b0, b1, length);
	} else {
		ot->recv_rot(r, b, length);
	}
	io->flush();
	long long t = time_from(start);

	// // Verification
	// if (party == ALICE) {
	// 	io->send_block(b0, length);
	// 	io->send_block(b1, length);
	// } else if (party == BOB) {
	// 	io->recv_block(b0, length);
	// 	io->recv_block(b1, length);
	// 	for (int64_t i = 0; i < length; ++i) {
	// 		if (b[i])
	// 			assert(cmpBlock(&r[i], &b1[i], 1));
	// 		else
	// 			assert(cmpBlock(&r[i], &b0[i], 1));
	// 	}
	// }
	
	io->flush();
	// TODO: copy LSB of b0, b1 and r into the array that is returned
	if (party == ALICE) {
		// OT sender
		for(int64_t i = 0; i < length; i++) {
			m0[i] = _mm_extract_epi8(b0[i], 0);
			m1[i] = _mm_extract_epi8(b1[i], 0);
		}
	}
	else {
		// OT receiver
		for(int64_t i = 0; i < length; i++) {
			m0[i] = _mm_extract_epi8(r[i], 0);
			choice[i] = b[i];
		}
	}
	
	delete[] b0;
	delete[] b1;
	delete[] r;
	delete[] b;
	return t;
}

// Actively secure random OT
// if party == 1 (ALICE), we are hosting, so `remote_addr` can be nullptr
uint64_t random_ot(int party, const char* remote_addr, int port, long long int count, int mode, unsigned char* data0, unsigned char* data1) {
	NetIO * io = new NetIO(party==ALICE ? nullptr:remote_addr, port);
	uint64_t counter_start = io->counter;
	if(mode == 0){
		// std::cout<<"IKNP"<<std::endl;
		// Use IKNP
		// Second arg in constructor is true for malicious security
		IKNP<NetIO> * iknp = new IKNP<NetIO>(io, true);
		if (party == ALICE) {
			// OT sender
			rot<IKNP<NetIO>>(iknp, io, party, count, data0, data1, nullptr);
		}
		else {
			// OT receiver
			rot<IKNP<NetIO>>(iknp, io, party, count, data0, nullptr, data1);
		}
	
		delete iknp;
	}
	else{
		// std::cout<<"Ferret"<<std::endl;
		// Use Ferret
		FerretCOT<NetIO> * ferretcot = new FerretCOT<NetIO>(party, threads, &io, true, true, "data/" + to_string(port));
		if (party == ALICE) {
			// OT sender
			rot<FerretCOT<NetIO>>(ferretcot, io, party, count, data0, data1, nullptr);
		}
		else {
			// OT receiver
			rot<FerretCOT<NetIO>>(ferretcot, io, party, count, data0, nullptr, data1);
		}
		std::cout<<"Ferret done"<<std::endl;
		delete ferretcot;
		
	}

	// return number of bytes sent
	uint64_t num_bytes = io->counter - counter_start;
	delete io;
	return num_bytes;
	
}

//void playground_func() {
//	// let's test f2k
//	const block a = _mm_set_epi64x(0xdeadbeef12345678, 0xabcdef0123456789);
//	const block b = _mm_set_epi64x(0x1926371029371ab1, 0x928dfa02719a8c9d);
//
//	block r1;
//	block r2;
//
//	mul128(a, b, &r1, &r2);
//
//	// print a and b
//	std::cout << "a = " << a << std::endl;
//	std::cout << "b = " << b << std::endl;
//
//	// print r1 and r2
//	std::cout << "r1 = " << r1 << std::endl;
//	std::cout << "r2 = " << r2 << std::endl;
//
//}