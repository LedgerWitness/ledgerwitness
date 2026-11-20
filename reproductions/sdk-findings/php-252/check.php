<?php
// Soneso/stellar-php-sdk#252: a SAC deployed with CREATE_CONTRACT_V2 is
// re-encoded as CREATE_CONTRACT, so its hash is wrong.
// Input: testnet transaction 11446413... (ledger 5822970, SUCCESS), as applied.
require __DIR__ . '/vendor/autoload.php';

use Soneso\StellarSDK\AbstractTransaction;
use Soneso\StellarSDK\Network;

const ENVELOPE = 'AAAAAgAAAAAMCf4TNV9jU4d5QPRyB0MihH9zgsUW6keMIZsIAP/UDgABIWIAWNn5AAAAAQAAAAEAAAAAAAAAAAAAAABq/6QvAAAAAAAAAAEAAAAAAAAAGAAAAAMAAAABAAAAAUxXVjIAAAAADAn+EzVfY1OHeUD0cgdDIoR/c4LFFupHjCGbCAD/1A4AAAABAAAAAAAAAAAAAAABAAAAAAAAAAAAAAABAAAABgAAAAFv3MrH8x/kbUTlOq9FidhQHGBb+ZN/BNe3GCwznhvb8wAAABQAAAABAAKrMQAAAAAAAAHcAAAAAAABIP4AAAABAP/UDgAAAEBDm1gYDzIAJHtXc0hWqix7I22s4B9ga7eWJp5+8eWxuT/608SPbN9y5hzli1R2NX9ARlcA5fNgcEUnW1Y6v8sD';
const NETWORK_HASH = '11446413b5a7a95f8bb18e616a5187f19a5a7b146fdef3c5c2c77e5ea87aaadc';

$tx = AbstractTransaction::fromEnvelopeBase64XdrString(ENVELOPE);
$hash = bin2hex($tx->hash(Network::testnet()));
echo "sdk hash     $hash\n";
echo "network hash " . NETWORK_HASH . "\n";
echo "re-encoded envelope identical: " . var_export($tx->toEnvelopeXdrBase64() === ENVELOPE, true) . "\n";
exit($hash === NETWORK_HASH ? 0 : 1);
