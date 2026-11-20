// Java conformance runner: computes the probes in docs/spec/probes.md for
// every corpus entry with java-stellar-sdk (network.lightsail:stellar-sdk).
//
// Envelope probes go through the SDK's transaction API
// (AbstractTransaction.fromEnvelopeXdr and its parsed operations), the layer
// applications use; meta probes read the SDK's decoded XDR objects. The logic
// mirrors runners/javascript/runner.mjs so that a difference reflects the SDK.
//
// Usage: java -cp 'runners/java/.deps/*' runners/java/Runner.java <corpus-entries-dir>

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import java.util.stream.Stream;
import org.stellar.sdk.AbstractTransaction;
import org.stellar.sdk.FeeBumpTransaction;
import org.stellar.sdk.MuxedAccount;
import org.stellar.sdk.Network;
import org.stellar.sdk.SignerKey;
import org.stellar.sdk.StrKey;
import org.stellar.sdk.Transaction;
import org.stellar.sdk.operations.AccountMergeOperation;
import org.stellar.sdk.operations.ChangeTrustOperation;
import org.stellar.sdk.operations.Operation;
import org.stellar.sdk.operations.PathPaymentStrictReceiveOperation;
import org.stellar.sdk.operations.PathPaymentStrictSendOperation;
import org.stellar.sdk.operations.PaymentOperation;
import org.stellar.sdk.xdr.AccountID;
import org.stellar.sdk.xdr.AssetType;
import org.stellar.sdk.xdr.LedgerCloseMeta;
import org.stellar.sdk.xdr.LedgerEntry;
import org.stellar.sdk.xdr.LedgerEntryChange;
import org.stellar.sdk.xdr.LedgerEntryType;
import org.stellar.sdk.xdr.LedgerHeader;
import org.stellar.sdk.xdr.OperationMeta;
import org.stellar.sdk.xdr.OperationMetaV2;
import org.stellar.sdk.xdr.TransactionEnvelope;
import org.stellar.sdk.xdr.TransactionMeta;
import org.stellar.sdk.xdr.TransactionResultCode;
import org.stellar.sdk.xdr.TransactionResultMeta;
import org.stellar.sdk.xdr.TransactionResultMetaV1;
import org.stellar.sdk.xdr.TrustLineAsset;

public class Runner {
  static final HexFormat HEX = HexFormat.of();
  static final List<String> ENVELOPE_PROBES =
      List.of("tx_hash", "tx_source", "tx_source_muxed_id", "op_types", "op_sources", "muxed_ids", "envelope_pool_ids");

  static String library() {
    String v = Transaction.class.getPackage().getImplementationVersion();
    return "java-stellar-sdk " + (v != null ? v : jarVersion());
  }

  // The jar manifest may not carry a version; fall back to the jar's file name.
  static String jarVersion() {
    String p = Transaction.class.getProtectionDomain().getCodeSource().getLocation().getPath();
    String name = Paths.get(p).getFileName().toString();
    return name.replaceFirst("^stellar-sdk-", "").replaceFirst("\\.jar$", "");
  }

  static List<Path> findEntries(Path root) throws IOException {
    try (Stream<Path> s = Files.walk(root)) {
      return s.filter(p -> p.getFileName().toString().equals("entry.json")).sorted().toList();
    }
  }

  static String sha256(byte[] b) throws Exception {
    return HEX.formatHex(MessageDigest.getInstance("SHA-256").digest(b));
  }

  static String muxedId(String address) {
    return address != null && address.startsWith("M") ? new MuxedAccount(address).getMuxedId().toString() : null;
  }

  static String accountStrkey(AccountID a) {
    return StrKey.encodeEd25519PublicKey(a.getAccountID().getEd25519().getUint256());
  }

  static String assetCode(byte[] code) {
    int n = code.length;
    while (n > 0 && code[n - 1] == 0) n--;
    return new String(code, 0, n, StandardCharsets.ISO_8859_1);
  }

  static Map<String, Object> errorProbe(Throwable e) {
    String m = String.valueOf(e.getMessage() != null ? e.getMessage() : e.toString());
    return Map.of("error", m.length() > 200 ? m.substring(0, 200) : m);
  }

  static String destination(Operation op) {
    if (op instanceof PaymentOperation p) return p.getDestination();
    if (op instanceof PathPaymentStrictReceiveOperation p) return p.getDestination();
    if (op instanceof PathPaymentStrictSendOperation p) return p.getDestination();
    if (op instanceof AccountMergeOperation p) return p.getDestination();
    return null;
  }

  static Map<String, Object> envelopeProbes(byte[] raw, String passphrase) throws Exception {
    AbstractTransaction parsed =
        AbstractTransaction.fromEnvelopeXdr(TransactionEnvelope.fromXdrByteArray(raw), new Network(passphrase));
    String hash = parsed.hashHex();
    Transaction tx = parsed instanceof FeeBumpTransaction fb ? fb.getInnerTransaction() : (Transaction) parsed;
    List<String> ids = new ArrayList<>();
    List<String> pools = new ArrayList<>();
    List<String> types = new ArrayList<>();
    List<String> sources = new ArrayList<>();
    String txSource = tx.getSourceAccount();
    if (muxedId(txSource) != null) ids.add(muxedId(txSource));
    for (Operation op : tx.getOperations()) {
      String source = op.getSourceAccount();
      if (muxedId(source) != null) ids.add(muxedId(source));
      String dest = destination(op);
      if (muxedId(dest) != null) ids.add(muxedId(dest));
      if (op instanceof ChangeTrustOperation ct && ct.getAsset().getAssetType() == AssetType.ASSET_TYPE_POOL_SHARE) {
        try {
          pools.add(ct.getAsset().getLiquidityPool().getLiquidityPoolId());
        } catch (Exception e) {
          pools.add("error: " + e.getMessage());
        }
      }
      types.add(op.toXdr().getBody().getDiscriminant().name());
      sources.add(source);
    }
    Map<String, Object> out = new LinkedHashMap<>();
    out.put("tx_hash", hash);
    out.put("tx_source", txSource);
    out.put("tx_source_muxed_id", muxedId(txSource));
    out.put("op_types", types);
    out.put("op_sources", sources);
    out.put("muxed_ids", ids);
    out.put("envelope_pool_ids", pools);
    return out;
  }

  record OpMeta(LedgerEntryChange[] changes, int events) {}

  static List<OpMeta> operationMeta(TransactionMeta meta) {
    List<OpMeta> out = new ArrayList<>();
    switch (meta.getDiscriminant()) {
      case 4 -> {
        for (OperationMetaV2 o : meta.getV4().getOperations())
          out.add(new OpMeta(o.getChanges().getLedgerEntryChanges(), o.getEvents().length));
      }
      case 3 -> {
        OperationMeta[] ops = meta.getV3().getOperations();
        var soroban = meta.getV3().getSorobanMeta();
        int n = soroban != null && ops.length == 1 ? soroban.getEvents().length : 0;
        for (OperationMeta o : ops) out.add(new OpMeta(o.getChanges().getLedgerEntryChanges(), n));
      }
      default -> {
        OperationMeta[] ops =
            switch (meta.getDiscriminant()) {
              case 2 -> meta.getV2().getOperations();
              case 1 -> meta.getV1().getOperations();
              default -> meta.getOperations();
            };
        for (OperationMeta o : ops) out.add(new OpMeta(o.getChanges().getLedgerEntryChanges(), 0));
      }
    }
    return out;
  }

  static String trustlineAsset(TrustLineAsset a) {
    return switch (a.getDiscriminant()) {
      case ASSET_TYPE_POOL_SHARE -> "pool:" + HEX.formatHex(a.getLiquidityPoolID().getPoolID().getHash());
      case ASSET_TYPE_CREDIT_ALPHANUM4 ->
          assetCode(a.getAlphaNum4().getAssetCode().getAssetCode4()) + ":" + accountStrkey(a.getAlphaNum4().getIssuer());
      case ASSET_TYPE_CREDIT_ALPHANUM12 ->
          assetCode(a.getAlphaNum12().getAssetCode().getAssetCode12()) + ":" + accountStrkey(a.getAlphaNum12().getIssuer());
      default -> "native";
    };
  }

  record TxProcessing(byte[] hash, TransactionResultCode code, TransactionMeta meta) {}

  static void ledgerProbes(LedgerCloseMeta lcm, String focusHash, Map<String, Object> ledger, Map<String, Object> focus) {
    LedgerHeader header;
    List<TxProcessing> txs = new ArrayList<>();
    switch (lcm.getDiscriminant()) {
      case 0 -> {
        header = lcm.getV0().getLedgerHeader().getHeader();
        for (TransactionResultMeta p : lcm.getV0().getTxProcessing())
          txs.add(new TxProcessing(p.getResult().getTransactionHash().getHash(),
              p.getResult().getResult().getResult().getDiscriminant(), p.getTxApplyProcessing()));
      }
      case 1 -> {
        header = lcm.getV1().getLedgerHeader().getHeader();
        for (TransactionResultMeta p : lcm.getV1().getTxProcessing())
          txs.add(new TxProcessing(p.getResult().getTransactionHash().getHash(),
              p.getResult().getResult().getResult().getDiscriminant(), p.getTxApplyProcessing()));
      }
      case 2 -> {
        header = lcm.getV2().getLedgerHeader().getHeader();
        for (TransactionResultMetaV1 p : lcm.getV2().getTxProcessing())
          txs.add(new TxProcessing(p.getResult().getTransactionHash().getHash(),
              p.getResult().getResult().getResult().getDiscriminant(), p.getTxApplyProcessing()));
      }
      default -> throw new IllegalArgumentException("LedgerCloseMeta v" + lcm.getDiscriminant() + " is not supported");
    }
    ledger.put("ledger_sequence", header.getLedgerSeq().getUint32().getNumber());
    ledger.put("protocol_version", header.getLedgerVersion().getUint32().getNumber());
    ledger.put("tx_count", txs.size());
    if (focusHash == null) return;
    TxProcessing tp = txs.stream().filter(p -> HEX.formatHex(p.hash()).equals(focusHash)).findFirst()
        .orElseThrow(() -> new IllegalArgumentException("focus transaction not in tx_processing"));
    TreeSet<String> signers = new TreeSet<>();
    List<String> trustlines = new ArrayList<>();
    List<Integer> events = new ArrayList<>();
    for (OpMeta om : operationMeta(tp.meta())) {
      events.add(om.events());
      for (LedgerEntryChange c : om.changes()) {
        LedgerEntry entry = c.getCreated() != null ? c.getCreated() : c.getUpdated() != null ? c.getUpdated() : c.getRestored();
        if (entry == null) continue;
        var d = entry.getData();
        if (d.getDiscriminant() == LedgerEntryType.ACCOUNT) {
          for (var s : d.getAccount().getSigners()) signers.add(SignerKey.fromXdr(s.getKey()).getEncodedSignerKey());
        } else if (d.getDiscriminant() == LedgerEntryType.TRUSTLINE && c.getCreated() != null) {
          trustlines.add(accountStrkey(d.getTrustLine().getAccountID()) + "|" + trustlineAsset(d.getTrustLine().getAsset()));
        }
      }
    }
    trustlines.sort(null);
    focus.put("successful", tp.code() == TransactionResultCode.txSUCCESS || tp.code() == TransactionResultCode.txFEE_BUMP_INNER_SUCCESS);
    focus.put("meta_signer_keys", new ArrayList<>(signers));
    focus.put("meta_created_trustlines", trustlines);
    focus.put("op_contract_event_counts", events);
  }

  // Sweep sets share one ledger among many transactions: keep the last
  // decoded ledger and its round-trip result.
  static String cachedKey;
  static LedgerCloseMeta cachedLedger;
  static Boolean cachedRoundtrip;

  static Map<String, Object> runEntry(Path path) throws Exception {
    byte[] manifestBytes = Files.readAllBytes(path);
    JsonObject entry = JsonParser.parseString(new String(manifestBytes, StandardCharsets.UTF_8)).getAsJsonObject();
    Map<String, Object> inputs = new LinkedHashMap<>();
    inputs.put("manifest_sha256", sha256(manifestBytes));
    Path dir = path.getParent();
    long started = System.nanoTime();
    Map<String, Object> result = new LinkedHashMap<>();
    result.put("id", entry.get("id").getAsString());
    for (String k : List.of("decode", "error", "roundtrip_exact", "envelope_decode", "envelope_roundtrip_exact")) result.put(k, null);
    Map<String, Object> probes = new LinkedHashMap<>();
    result.put("probes", probes);
    List<String> errors = new ArrayList<>();
    Map<String, Object> focus = new LinkedHashMap<>();

    if (entry.has("xdr_type") && entry.get("xdr_type").getAsString().equals("LedgerCloseMeta")) {
      byte[] raw = Files.readAllBytes(dir.resolve(entry.get("xdr_path").getAsString()));
      String key = sha256(raw);
      inputs.put("ledger_sha256", key);
      LedgerCloseMeta lcm = null;
      if (key.equals(cachedKey)) {
        lcm = cachedLedger;
        result.put("decode", "ok");
      } else {
        try {
          lcm = LedgerCloseMeta.fromXdrByteArray(raw);
          result.put("decode", "ok");
          cachedKey = key;
          cachedLedger = lcm;
          cachedRoundtrip = Arrays.equals(lcm.toXdrByteArray(), raw);
        } catch (StackOverflowError | OutOfMemoryError e) {
          result.put("decode", "panic");
          errors.add("ledger: " + e);
        } catch (Exception e) {
          result.put("decode", "error");
          errors.add("ledger: " + e.getMessage());
        }
      }
      if (lcm != null) {
        result.put("roundtrip_exact", cachedRoundtrip);
        try {
          JsonObject f = entry.has("focus") && entry.get("focus").isJsonObject() ? entry.getAsJsonObject("focus") : null;
          String focusHash = f != null && f.has("tx_hash") && !f.get("tx_hash").isJsonNull() ? f.get("tx_hash").getAsString() : null;
          ledgerProbes(lcm, focusHash, probes, focus);
        } catch (Exception e) {
          errors.add("ledger probes: " + e.getMessage());
        }
      }
    }

    String derived = null;
    if (entry.has("derived_artifacts")) {
      for (var a : entry.getAsJsonArray("derived_artifacts")) {
        JsonObject o = a.getAsJsonObject();
        if (o.has("derived_from") && o.get("derived_from").getAsString().equals("focus_envelope")) {
          derived = o.get("xdr_path").getAsString();
          break;
        }
      }
    }
    if (derived != null) {
      byte[] raw = Files.readAllBytes(dir.resolve(derived));
      inputs.put("envelope_sha256", sha256(raw));
      try {
        TransactionEnvelope env = TransactionEnvelope.fromXdrByteArray(raw);
        result.put("envelope_decode", "ok");
        result.put("envelope_roundtrip_exact", Arrays.equals(env.toXdrByteArray(), raw));
      } catch (Exception e) {
        result.put("envelope_decode", "error");
        errors.add("envelope: " + e.getMessage());
      }
      if ("ok".equals(result.get("envelope_decode"))) {
        try {
          focus.putAll(envelopeProbes(raw, entry.get("network_passphrase").getAsString()));
        } catch (Exception e) {
          // The raw XDR decoded but the SDK's transaction API rejected it.
          errors.add("envelope probes: " + e.getMessage());
          for (String k : ENVELOPE_PROBES) focus.put(k, errorProbe(e));
        }
      }
    }
    if (!focus.isEmpty()) probes.put("focus", focus);
    if (!errors.isEmpty()) result.put("error", String.join("; ", errors));
    result.put("inputs", inputs);
    result.put("elapsed_ms", Math.round((System.nanoTime() - started) / 1e5) / 10.0);
    return result;
  }

  public static void main(String[] args) throws Exception {
    Path root = Paths.get(args.length > 0 ? args[0] : "corpus/entries");
    List<Object> results = new ArrayList<>();
    for (Path p : findEntries(root)) {
      try {
        results.add(runEntry(p));
      } catch (Throwable e) {
        // Report under the entry's own ID so the matrix can score it.
        String id;
        try {
          id = JsonParser.parseString(Files.readString(p)).getAsJsonObject().get("id").getAsString();
        } catch (Exception ignored) {
          id = p.getParent().getFileName().toString();
        }
        Map<String, Object> r = new LinkedHashMap<>();
        r.put("id", id);
        r.put("decode", "panic");
        r.put("error", String.valueOf(e));
        for (String k : List.of("roundtrip_exact", "envelope_decode", "envelope_roundtrip_exact")) r.put(k, null);
        r.put("probes", Map.of());
        results.add(r);
      }
    }
    Map<String, Object> out = new LinkedHashMap<>();
    out.put("runner", "java");
    out.put("library", library());
    out.put("probe_spec", "0.2.0");
    out.put("results", results);
    Gson gson = new GsonBuilder().serializeNulls().setPrettyPrinting().disableHtmlEscaping().create();
    System.out.println(gson.toJson(out));
  }
}
