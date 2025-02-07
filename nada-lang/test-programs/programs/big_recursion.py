
from nada_dsl import *

INPUTS = 70

def nada_main():

    party_one = Party(name="PartyOne")

    secrets = [
    SecretInteger(Input(name=f"int_{str(i)}", party=party_one)) for i in range(INPUTS)
    ]

    sum_output = secrets[0] + secrets[1] + secrets[2] + secrets[3] + secrets[4] + secrets[5] + secrets[6] + secrets[7] + secrets[8] + secrets[9] + secrets[10] + secrets[11] + secrets[12] + secrets[13] + secrets[14] + secrets[15] + secrets[16] + secrets[17] + secrets[18] + secrets[19] + secrets[20] + secrets[21] + secrets[22] + secrets[23] + secrets[24] + secrets[25] + secrets[26] + secrets[27] + secrets[28] + secrets[29] + secrets[30] + secrets[31] + secrets[32] + secrets[33] + secrets[34] + secrets[35] + secrets[36] + secrets[37] + secrets[38] + secrets[39] + secrets[40] + secrets[41] + secrets[42] + secrets[43] + secrets[44] + secrets[45] + secrets[46] + secrets[47] + secrets[48] + secrets[49] + secrets[50] + secrets[51] + secrets[52] + secrets[53] + secrets[54] + secrets[55] + secrets[56] + secrets[57] + secrets[58] + secrets[59] + secrets[60] + secrets[61] + secrets[62] + secrets[63] + secrets[64] + secrets[65] + secrets[66] + secrets[67] + secrets[68] + secrets[69]
    sum_output = sum_output + sum_output + sum_output + sum_output + sum_output


    return [Output(sum_output, "output", party_one)]