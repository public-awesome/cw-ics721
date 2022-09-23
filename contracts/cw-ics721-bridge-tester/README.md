This contract is intented to a counterpart to the `cw-ics721-bridge`
contract that ought to have been distributed along with this source
code. It is intended to be used to answer the following questions:

How does the ics721-bridge contract respond if..

- the other side closes the connection?
- the other side sends a class ID corresponding to a class ID that is
  valid on a different channel but not on its channel.
- the other side sends IBC messages where the..
  - class ID is empty?
  - token URIs and IDs have different lengths?
  - class metadata sent over does not match existing metadata?
- two identical messages are sent to the source chain in short
  succesion (before relaying of ACKS)? In this case, the first NFT
  should be "unwraped" and a debt voucher issued for the other one.
- two of the same token IDs are sent in one transfer message?
- the same token is sent twice? First should work, second should fail.

