// Beans-BV/dotnet-stellar-sdk#281: a PRECOND_V2 transaction whose fields are
// at their defaults is re-encoded as PRECOND_TIME, so its hash is wrong.
// Input: mainnet transaction b4a6d6a2... (ledger 55000000), as applied.
using StellarDotnetSdk;
using StellarDotnetSdk.Transactions;

const string Envelope = "AAAAAgAAAAAjIV2QSjkjVC2NRbEBkV2PJmIBq2UKJLQ8WtBuCDXJ2wAAAfQCynanAAIn3gAAAAIAAAABAAAAAAAAAAAAAAAAZ21qPwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAAAAAQAAAAA7ngKfAUS0/FGZa4KJx3HXU6lwFOcBJPM4wex7LzuKWQAAAAIAAAAAAAAAAACVE5oAAAAAO54CnwFEtPxRmWuCicdx11OpcBTnASTzOMHsey87ilkAAAABeVhMTQAAAAAiNtdw/g0+AVIZddXPesSEK3CV1rGp12jT6aYy6ZkGvgAAAAAAlROaAAAAAgAAAAFQWUJDAAAAAGoebauqH6pI95KR+EBZVmms+p4HXgOxFvrW2rIJm6+gAAAAAUdPTEQAAAAARdyZXmQpu5u4/qV3g+/setPieuf7JpEh1TZIeT30F1MAAAAAAAAAAgg1ydsAAABA4b8Q29wH2OARp5SSjIFp68OG8fXL40Ds16TjPsspD1GqyAx4GDfjfZ+IEeSnYa7Qr/xzCdVk0fyJh4qwfHBFAi87ilkAAABAuutRndzo0SxfqbLK4IYQFwOr0blwGWSv4RMDO8x9uYwESqNHCtL43vMIjrpB3/irBxisBvjqMwATggWhAB4wCA==";
const string NetworkHash = "b4a6d6a2dfc62268a5f864f51d761207641e0c72bd8eb277159005f0f3503db9";

var tx = Transaction.FromEnvelopeXdr(Envelope);
var hash = Convert.ToHexString(tx.Hash(new Network("Public Global Stellar Network ; September 2015"))).ToLowerInvariant();
Console.WriteLine($"sdk hash     {hash}");
Console.WriteLine($"network hash {NetworkHash}");
Console.WriteLine($"re-encoded envelope identical: {tx.ToEnvelopeXdrBase64() == Envelope}");
return hash == NetworkHash ? 0 : 1;
