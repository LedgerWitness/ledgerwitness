// Soneso/stellar_flutter_sdk#305: a SAC deployed with CREATE_CONTRACT_V2 is
// re-encoded as CREATE_CONTRACT, so its hash is wrong.
// Input: testnet transaction 11446413... (ledger 5822970, SUCCESS), as applied.
import 'dart:io';

import 'package:stellar_flutter_sdk/stellar_flutter_sdk.dart';

const envelope = 'AAAAAgAAAAAMCf4TNV9jU4d5QPRyB0MihH9zgsUW6keMIZsIAP/UDgABIWIAWNn5AAAAAQAAAAEAAAAAAAAAAAAAAABq/6QvAAAAAAAAAAEAAAAAAAAAGAAAAAMAAAABAAAAAUxXVjIAAAAADAn+EzVfY1OHeUD0cgdDIoR/c4LFFupHjCGbCAD/1A4AAAABAAAAAAAAAAAAAAABAAAAAAAAAAAAAAABAAAABgAAAAFv3MrH8x/kbUTlOq9FidhQHGBb+ZN/BNe3GCwznhvb8wAAABQAAAABAAKrMQAAAAAAAAHcAAAAAAABIP4AAAABAP/UDgAAAEBDm1gYDzIAJHtXc0hWqix7I22s4B9ga7eWJp5+8eWxuT/608SPbN9y5hzli1R2NX9ARlcA5fNgcEUnW1Y6v8sD';
const networkHash = '11446413b5a7a95f8bb18e616a5187f19a5a7b146fdef3c5c2c77e5ea87aaadc';

void main() {
  final tx = AbstractTransaction.fromEnvelopeXdrString(envelope) as Transaction;
  final hash = Util.bytesToHex(tx.hash(Network.TESTNET)).toLowerCase();
  print('sdk hash     $hash');
  print('network hash $networkHash');
  print('re-encoded envelope identical: ${tx.toEnvelopeXdrBase64() == envelope}');
  exit(hash == networkHash ? 0 : 1);
}
