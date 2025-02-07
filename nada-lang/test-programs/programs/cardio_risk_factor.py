from nada_dsl import *

'''
Viand, Alexander, Patrick Jattke, and Anwar Hithnawi. "SoK: Fully homomorphic
encryption compilers." 2021 IEEE Symposium on Security and Privacy (SP).

Cardio risk factor assessment:
    Check 1) +1 if man && age > 50 years
    Check 2) +1 if woman && age > 60 years
    Check 3) +1 if smoking
    Check 4) +1 if diabetic
    Check 5) +1 if high blood pressure
    Check 6) +1 if HDL cholesterol < 40
    Check 7) +1 if weight > height-90
    Check 8) +1 if daily physical activity < 30
    Check 9) +1 if man && alcohol cons. > 3 glasses/day
    Check 10) +1 if !man && alcohol cons. > 2 glasses/day
'''

def nada_main():
    party1 = Party(name="Party1")

    # Note: This should be Boolean: 0 for Male, 1 for Female.
    sex = SecretUnsignedInteger(Input(name="sex", party=party1))
    age = SecretUnsignedInteger(Input(name="age", party=party1))
    # Note: This should be Boolean: 0 for non-smoker, 1 for smoker.
    is_smoking = SecretUnsignedInteger(Input(name="is_smoking", party=party1))
    # Note: This should be Boolean: 0 for non-diabetic, 1 for diabetic.
    is_diabetic = SecretUnsignedInteger(Input(name="is_diabetic", party=party1))
    # Note: This should be Boolean: 0 for low blood pressure, 1 for high blood pressure.
    high_blood_pressure = SecretUnsignedInteger(Input(name="high_blood_pressure", party=party1))
    hdl_cholesterol = SecretUnsignedInteger(Input(name="hdl_cholesterol", party=party1))
    height = SecretUnsignedInteger(Input(name="height", party=party1))
    weight = SecretUnsignedInteger(Input(name="weight", party=party1))
    physical_act = SecretUnsignedInteger(Input(name="physical_act", party=party1))
    # Note: This is in glasses per day.
    drinking = SecretUnsignedInteger(Input(name="drinking", party=party1))

    risk_score = UnsignedInteger(0)

    # Check 1) +1 if man && age > 50 years
    risk_score = risk_score + (sex < UnsignedInteger(1)).if_else(
        (age > UnsignedInteger(50)).if_else(UnsignedInteger(1), UnsignedInteger(0)),
        UnsignedInteger(0)
    )

    # Check 2) +1 if woman && age > 60 years
    risk_score = risk_score + (sex > UnsignedInteger(0)).if_else(
        (age > UnsignedInteger(60)).if_else(UnsignedInteger(1), UnsignedInteger(0)),
        UnsignedInteger(0)
    )

    # Check 3) +1 if smoking
    risk_score = risk_score + is_smoking

    # Check 4) +1 if diabetic
    risk_score = risk_score + is_diabetic

    # Check 5) +1 if high blood pressure
    risk_score = risk_score + high_blood_pressure

    # Check 6) +1 if HDL cholesterol < 40
    risk_score = risk_score + (hdl_cholesterol < UnsignedInteger(40)).if_else(
        UnsignedInteger(1), UnsignedInteger(0)
    )

    # Check 7) +1 if weight > height-90
    risk_score = risk_score + (weight > (height - UnsignedInteger(90))).if_else(
        UnsignedInteger(1), UnsignedInteger(0)
    )

    # Check 8) +1 if daily physical activity < 30
    risk_score = risk_score + (physical_act < UnsignedInteger(30)).if_else(
        UnsignedInteger(1), UnsignedInteger(0)
    )

    # Check 9) +1 if man && alcohol cons. > 3 glasses/day
    risk_score = risk_score + (sex < UnsignedInteger(1)).if_else(
        (drinking > UnsignedInteger(3)).if_else(UnsignedInteger(1), UnsignedInteger(0)),
        UnsignedInteger(0)
    )

    # Check 10) +1 if !man && alcohol cons. > 2 glasses/day
    risk_score = risk_score + (sex > UnsignedInteger(0)).if_else(
        (drinking > UnsignedInteger(2)).if_else(UnsignedInteger(1), UnsignedInteger(0)),
        UnsignedInteger(0)
    )

    return [Output(risk_score, "my_output", party1)]
