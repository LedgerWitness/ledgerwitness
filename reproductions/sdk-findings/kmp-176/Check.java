// Soneso/kmp-stellar-sdk#176: fromEnvelopeXdr refuses a successful
// ExtendFootprintTTL transaction whose extendTo is 0.
// Input: testnet transaction 21f5b9f4... (ledger 3619212, txSUCCESS), the
// focus envelope of corpus entry core.testnet.cap46.extend-ttl-extend-to-zero.p27.
//
// Usage: ../kmp-175/fetch.sh && java -cp '../kmp-175/.deps/*' Check.java

import com.soneso.stellar.sdk.AbstractTransaction;
import com.soneso.stellar.sdk.Network;
import java.nio.file.Files;
import java.nio.file.Path;

public class Check {
  public static void main(String[] args) throws Exception {
    byte[] bytes = Files.readAllBytes(Path.of(
        "../../../corpus/entries/core-emitted/core.testnet.cap46.extend-ttl-extend-to-zero.p27/focus-envelope.xdr"));
    try {
      AbstractTransaction.Companion.fromEnvelopeXdr(bytes, new Network("Test SDF Network ; September 2015"));
      System.out.println("parsed");
    } catch (IllegalArgumentException e) {
      System.out.println("refused: " + e.getMessage());
      System.exit(1);
    }
  }
}
