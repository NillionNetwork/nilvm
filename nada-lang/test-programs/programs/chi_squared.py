from nada_dsl import *

'''
K. Lauter, A. Lopez-Alt, and M. Naehrig, "Private Computation on Encrypted
Genomic Data," in LATINCRYPT 2014. https://eprint.iacr.org/2015/133

Given three secret integers N0, N1, and N2, the following program computes
alpha, beta_1, beta_2, and beta_3.

Since fixed/floating points are not supported, the client can compute
chi^2 = (alpha / (2*N)) * (1/beta_1 + 1/beta_2 + 1/beta_3).

def chi_squared(N0, N1, N2):
    alpha = (4*N0*N2 - N1**2)**2
    beta_1 = 2*(2*N0 + N1)**2
    beta_2 = (2*N0 + N1)*(2*N2 + N1)
    beta_3 = 2*(2*N2 + N1)**2
    print(alpha, beta_1, beta_2, beta_3)
'''

def nada_main():
    party1 = Party(name="Party1")
    n0 = SecretInteger(Input(name="n0", party=party1))
    n1 = SecretInteger(Input(name="n1", party=party1))
    n2 = SecretInteger(Input(name="n2", party=party1))

    # alpha = (4*N0*N2 - N1**2)**2
    # Note: replace n1 * n1 with **2 when we add support for power with secret base.
    four_n0_n2_minus_n1_sq = Integer(4) * n0 * n2 - n1 * n1
    # Note: replace four_n0_n2_minus_n1_sq * four_n0_n2_minus_n1_sq with **2
    # when we add support for power with secret base.
    alpha = four_n0_n2_minus_n1_sq * four_n0_n2_minus_n1_sq

    # beta_1 = 2*(2*N0 + N1)**2
    two_n0_plus_n1 = Integer(2) * n0 + n1
    # Note: replace two_n0_plus_n1 * two_n0_plus_n1 with **2 when we add support
    # for power with secret base.
    beta_1 = Integer(2) * two_n0_plus_n1 * two_n0_plus_n1

    # beta_2 = (2*N0 + N1)*(2*N2 + N1)
    two_n2_plus_n1 = Integer(2) * n2 + n1
    beta_2 = two_n0_plus_n1 * two_n2_plus_n1

    #  beta_3 = 2*(2*N2 + N1)**2
    # Note: replace two_n2_plus_n1 * two_n2_plus_n1 with **2 when we add support
    # for power with secret base.
    beta_3 = Integer(2) * two_n2_plus_n1 * two_n2_plus_n1

    return [
        Output(alpha, "alpha", party1),
        Output(beta_1, "beta_1", party1),
        Output(beta_2, "beta_2", party1),
        Output(beta_3, "beta_3", party1)
    ]
