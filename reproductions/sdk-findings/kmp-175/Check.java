// Soneso/kmp-stellar-sdk#175: a PRECOND_V2 transaction whose fields are at
// their defaults is re-encoded as PRECOND_TIME, so its hash is wrong.
// Input: mainnet transaction b4a6d6a2... (ledger 55000000), the focus
// envelope of corpus entry core.pubnet.cap21.precond-v2-default-fields.p22.
//
// Usage: ./fetch.sh && java -cp '.deps/*' Check.java

import com.soneso.stellar.sdk.AbstractTransaction;
import com.soneso.stellar.sdk.Network;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import kotlin.coroutines.EmptyCoroutineContext;
import kotlinx.coroutines.BuildersKt;

public class Check {
  static final String NETWORK_HASH = "b4a6d6a2dfc62268a5f864f51d761207641e0c72bd8eb277159005f0f3503db9";

  public static void main(String[] args) throws Exception {
    Path envelope = Path.of("../../../corpus/entries/core-emitted/core.pubnet.cap21.precond-v2-default-fields.p22/focus-envelope.xdr");
    byte[] bytes = Files.readAllBytes(envelope);
    AbstractTransaction tx = AbstractTransaction.Companion.fromEnvelopeXdr(
        bytes, new Network("Public Global Stellar Network ; September 2015"));
    // hashHex is a suspend function.
    String hash = (String) BuildersKt.runBlocking(EmptyCoroutineContext.INSTANCE, (scope, cont) -> tx.hashHex(cont));
    byte[] reencoded = java.util.Base64.getDecoder().decode(tx.toEnvelopeXdrBase64());
    System.out.println("sdk hash     " + hash);
    System.out.println("network hash " + NETWORK_HASH);
    System.out.println("re-encoded envelope identical: " + Arrays.equals(reencoded, bytes)
        + " (" + reencoded.length + " vs " + bytes.length + " bytes)");
    System.exit(hash.equals(NETWORK_HASH) ? 0 : 1);
  }
}
