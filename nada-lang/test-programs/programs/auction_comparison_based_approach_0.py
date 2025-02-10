from nada_dsl import *

def nada_main():

    bidder1 = Party(name="Bidder1")
    bidder2 = Party(name="Bidder2")
    bidder3 = Party(name="Bidder3")
    bidder4 = Party(name="Bidder4")
    bidder5 = Party(name="Bidder5")
    outparty = Party(name="OutParty")

    bid1 = SecretInteger(Input(name="bid1", party=bidder1))
    bid2 = SecretInteger(Input(name="bid2", party=bidder2))
    bid3 = SecretInteger(Input(name="bid3", party=bidder3))
    bid4 = SecretInteger(Input(name="bid4", party=bidder4))
    bid5 = SecretInteger(Input(name="bid5", party=bidder5))

    bidderId1 = SecretInteger(Input(name="bidder1", party=bidder1))
    bidderId2 = SecretInteger(Input(name="bidder2", party=bidder2))
    bidderId3 = SecretInteger(Input(name="bidder3", party=bidder3))
    bidderId4 = SecretInteger(Input(name="bidder4", party=bidder4))
    bidderId5 = SecretInteger(Input(name="bidder5", party=bidder5))

    max_bid = bid1
    max_bidder = bidderId1

    c = max_bid > bid2
    max_bid = c.if_else(max_bid, bid2)
    max_bidder = c.if_else(max_bidder, bidderId2)

    c = max_bid > bid3
    max_bid = c.if_else(max_bid, bid3)
    max_bidder = c.if_else(max_bidder, bidderId3)

    c = max_bid > bid4
    max_bid = c.if_else(max_bid, bid4)
    max_bidder = c.if_else(max_bidder, bidderId4)

    c = max_bid > bid5
    max_bid = c.if_else(max_bid, bid2)
    max_bidder = c.if_else(max_bidder, bidderId5)

    max_bid = Output(max_bid, "winning_bid", outparty)
    max_bidder = Output(max_bidder, "winner", outparty)

    return [max_bid, max_bidder]