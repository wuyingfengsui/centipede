
/*
escrow-recovery

Copyright 2018 by Kzen Networks

This file is part of escrow-recovery library
(https://github.com/KZen-networks/cryptography-utils)

Escrow-recovery is free software: you can redistribute
it and/or modify it under the terms of the GNU General Public
License as published by the Free Software Foundation, either
version 3 of the License, or (at your option) any later version.

@license GPL-3.0+ <https://github.com/KZen-networks/escrow-recovery/blob/master/LICENSE>
*/



use cryptography_utils::{FE,GE,BigInt};
use cryptography_utils::cryptographic_primitives::proofs::sigma_correct_homomorphic_elgamal_encryption_of_dlog::{HomoELGamalDlogProof,hegdWitness,hegdStatement};
use cryptography_utils::cryptographic_primitives::proofs::sigma_correct_homomrphic_elgamal_enc::{HomoELGamalProof,hegWitness,hegStatement};
use cryptography_utils::cryptographic_primitives::hashing::hash_sha512::HSha512;
use cryptography_utils::cryptographic_primitives::hashing::traits::*;
use cryptography_utils::elliptic::curves::traits::*;
use bulletproof::proofs::range_proof::{RangeProof,generate_random_point};
use cryptography_utils::elliptic::curves::traits::*;
use std::ops::{Shr, Shl,Mul, Add};
use wallet::SecretShare;
use juggling::server::{hElGamal, Witness,hElGamalSegmented};
use Errors::{self, ErrorDecrypting};





pub struct mSegmentation;

impl mSegmentation{

    pub fn get_segment_k(secret: &FE, segment_size: &usize, k: u8) -> FE{
        let ss_bn = secret.to_big_int();
        let temp: FE = ECScalar::from(&ss_bn);
        let segment_size_u32 = segment_size.clone() as u32;
        let msb = segment_size_u32 * (k+1) as u32;
        let lsb = segment_size_u32 * k as u32;
        let two_bn = BigInt::from(2);
        let max = BigInt::pow(&two_bn,msb) - BigInt::from(1);
        let min = BigInt::pow(&two_bn,lsb) - BigInt::from(1);
        let mask = max - min;
        let segment_k_bn = mask & ss_bn;
        let segment_k_bn_rotated = BigInt::shr(segment_k_bn,(k * segment_size.clone() as u8) as usize);
        // println!("test = {:?}", test.to_str_radix(16));
        ECScalar::from(&segment_k_bn_rotated)
    }
    //returns r_k,{D_k,E_k}
    pub fn encrypt_segment_k(secret: &FE, random: &FE, segment_size: &usize, k: u8, pub_ke_y: &GE, G: &GE) -> hElGamal{
        let segment_k = mSegmentation::get_segment_k(secret,segment_size,k);
        let E_k = G.clone() * random;
        let r_kY = pub_ke_y.clone() * random;
        let x_kG = G.clone() * segment_k ;
        let D_k = r_kY + x_kG;

        hElGamal{D:D_k,E:E_k}
    }

    // TODO: find a way using generics to combine the following two fn's
    pub fn assemble_fe(segments: &Vec<FE>, segment_size: &usize) -> FE{
        let two = BigInt::from(2);
        let mut segments_2n = segments.clone();
        let seg1 = segments_2n.remove(0);
        let seg_sum = segments_2n.iter().zip(0..segments_2n.len()).fold(seg1,|acc,x|{
            let two_to_the_n = two.pow(segment_size.clone() as u32);
            let two_to_the_n_shifted = two_to_the_n.shl(x.1 * segment_size);
            let two_to_the_n_shifted_fe: FE = ECScalar::from(&two_to_the_n_shifted);
            let shifted_segment = x.0.clone() * two_to_the_n_shifted_fe;
            acc + shifted_segment
        });
        return seg_sum;
    }

    pub fn assemble_ge(segments: &Vec<GE>, segment_size: &usize) -> GE{
        let two = BigInt::from(2);
        let mut segments_2n = segments.clone();
        let seg1 = segments_2n.remove(0);
        let seg_sum = segments_2n.iter().zip(0..segments_2n.len()).fold(seg1,|acc,x|{
            let two_to_the_n = two.pow(segment_size.clone() as u32);
            let two_to_the_n_shifted = two_to_the_n.shl(x.1 * segment_size);
            let two_to_the_n_shifted_fe: FE = ECScalar::from(&two_to_the_n_shifted);
            let shifted_segment = x.0.clone() * two_to_the_n_shifted_fe;
            acc + shifted_segment
        });
        return seg_sum;
    }


    pub fn to_encrypted_segments(secret: &FE,  segment_size: &usize, num_of_segments: usize, pub_ke_y: &GE, G: &GE) -> (Witness, hElGamalSegmented){
        let r_vec =(0..num_of_segments).map(|_| {
            ECScalar::new_random()
        }).collect::<Vec<FE>>();
        let segmented_enc  =  (0..num_of_segments).map(|i|{
            //  let segment_i = mSegmentation::get_segment_k(secret,segment_size,i as u8);
            mSegmentation::encrypt_segment_k(secret, &r_vec[i], &segment_size, i as u8, pub_ke_y , G)
        }).collect::<Vec<hElGamal>>();
        let x_vec = (0..num_of_segments).map(|i|{
            mSegmentation::get_segment_k(secret, segment_size, i as u8)
        }).collect::<Vec<FE>>();
        let w = Witness{x_vec, r_vec};
        let heg_segmented = hElGamalSegmented{DE: segmented_enc};
        (w, heg_segmented)

    }

    //TODO: implement a more advance algorithm for dlog
    pub fn decrypt_segment(DE: &hElGamal, G: &GE ,private_key: &FE, segment_size: &usize) -> Result<FE,Errors>{
        let yE = DE.E.clone() * private_key;
        let D_minus_yE = DE.D.sub_point(&yE.get_element());
        // TODO: make bound bigger then 32
        let limit = 2u32.pow(segment_size.clone() as u32);
        let limit_bn = BigInt::from(2).pow(segment_size.clone() as u32);
        let mut test_fe:FE = ECScalar::from(&limit_bn);
        let one = BigInt::one();
        let one_fe : FE = ECScalar::from(&one);
        let mut result = Err(ErrorDecrypting);

        let mut test_fe: FE = ECScalar::from(&BigInt::one());
        let mut test_ge :GE = G.clone() * &test_fe;
        for i in 1..limit{

            if test_ge.get_element() == D_minus_yE.get_element(){
                result = Ok(test_fe.clone());
            }
            test_fe= ECScalar::from(&BigInt::from(i));
            test_ge = G.clone() * &test_fe;

        }
        result
    }

    pub fn decrypt(DE_vec: &hElGamalSegmented, G: &GE ,private_key: &FE, segment_size: &usize) -> FE{
        let limit = BigInt::from(2).pow(segment_size.clone() as u32);
        let vec_secret = (0..DE_vec.DE.len()).map(|i|{
            let result = mSegmentation::decrypt_segment(&DE_vec.DE[i],G,private_key, segment_size).expect("error decrypting");
            result
        }).collect::<Vec<FE>>();

        mSegmentation::assemble_fe(&vec_secret,&segment_size)
    }

}


#[cfg(test)]
mod tests {
    use cryptography_utils::BigInt;
    use cryptography_utils::{FE, GE};
    use juggling::segmentation::mSegmentation;
    use juggling::server::*;
    use cryptography_utils::elliptic::curves::traits::*;
    use wallet::SecretShare;

// TODO: test for 16bits
    /*
    #[test]
    fn test_m_segmentation() {
        let segment_size = 8;
        let y : FE = ECScalar::new_random();
        let G :GE= ECPoint::generator();
        let Y = G.clone() * &y;
        let x =  SecretShare::generate();
        let (segments, encryptions) = mSegmentation::to_encrypted_segments(&x.secret, &segment_size,32, &Y, &G );
        let secret_new = mSegmentation::assemble_fe(&segments.x_vec,&segment_size);
        let secret_decrypted = mSegmentation::decrypt(&encryptions, &G, &y, &segment_size);
        assert_eq!(x.secret.get_element(), secret_new.get_element());
        assert_eq!(x.secret.get_element(), secret_decrypted.get_element());

    }
    */
}