import { compile } from "handlebars";

// Load `.env` file
import "dotenv/config";

const DB_FILE = "data.json";
const DB_VERSION = 3;

const db: DataFile = await Bun.file(DB_FILE).json();
const flushDatabase = () => Bun.write(DB_FILE, JSON.stringify(db));

const HYPIXEL_API_KEY = process.env.HYPIXEL_API_KEY!;
if (!HYPIXEL_API_KEY) throw new Error("You need to provide the environnement variable `HYPIXEL_API_KEY`");
const PROFILE_UUID: Uuid = process.env.PROFILE_UUID! as Uuid
if (!PROFILE_UUID) throw new Error("You need to provide the environnement variable `PROFILE_UUID`");

interface DataFile {
  version: number,

  lastTransactionTimestamp: number;

  balance: number;
  maxBalance: number;

  drift: number;

  bankInterests: number;

  users: Record<Username, number>;
  operations: LocalOperation[];
}

interface LocalOperation {
  kind: LocalOperationKind;
  timestamp: number;

  amount?: number;
  username?: Username;
  // PlayerTransfer operation
  sender?: Username;
  repeatCount?: number;
}

enum LocalOperationKind {
  /**
    A player deposit or withdrawal operation from his purse.

    Associated amount is postive when the player deposited the money else negative.
    */
  PlayerPurse = "PLAYER_PURSE",
  /**
    A money transfer operation between two co-op members.
    */
  PlayerTransfer = "PLAYER_TRANSFER",

  /**
    A cash inflow from bank interests.
    */
  BankInterests = "BANK_INTERESTS",

  /**
    A marker in case 50 transactions were returned by the Hypixel API. That may
    mean that we missed transactions (indicated by the drift). You have to
    reconcilliate this by hand.
    */
  WeirdWaypoint = "WEIRD_WAYPOINT",
}

type Uuid = string & { readonly _sym: unique symbol; };
type Username = string & { readonly _sym: unique symbol; };
type StyledUsername = string & { readonly _sym: unique symbol; };
type HypixelResponse<T> = ({ success: true; } & T) | { success: false; cause: string };
type ProfileResponse = HypixelResponse<{ profile: Profile }>;

interface Profile {
  profile_id: string;
  community_upgrades: CommunityUpgrades;
  created_at: number;
  members: Record<Uuid, Member>;
  banking: Banking;
}

interface Member {
  leveling: Leveling
  // non-exhaustive
}

interface Leveling {
  completed_tasks: Array<string>;
  // non-exhaustive
}

interface CommunityUpgrades {
  // non-exhaustive
}

interface Banking {
  balance: number;
  transactions: Transaction[]
}

interface Transaction {
  amount: number;
  timestamp: number;
  action: TransactionAction;
  initiator_name: StyledUsername;
}

enum TransactionAction {
  Deposit = "DEPOSIT",
  Withdraw = "WITHDRAW",
}

let lastCheckTimestamp: number = 0;

async function fetchApi() {
  console.log(`Fetching fresh information for profile '${PROFILE_UUID}'`);

  let url = new URL("https://api.hypixel.net/v2/skyblock/profile");
  url.searchParams.append("key", HYPIXEL_API_KEY);
  url.searchParams.append("profile", PROFILE_UUID);

  let data: ProfileResponse = await fetch(url).then((res) => res.json());
  if (!data.success) throw new Error(`There was an error while fetching Hypixel's API: ${data.cause}`);

  updateTransactions(data.profile);
  lastCheckTimestamp = Date.now();

  console.log(`Got fresh information for profile '${PROFILE_UUID}', reloading clients`);

  server.publish("reload", "reload");
}

function getBankLevel(profile: Profile): number {
  const maxBalance: Record<string, number | undefined> = {
    BANK_UPGRADE_STARTER: 5_000_00,
    BANK_UPGRADE_GOLD: 100_000_000,
    BANK_UPGRADE_DELUXE: 250_000_000,
    BANK_UPGRADE_SUPER_DELUXE: 500_000_000,
    BANK_UPGRADE_PREMIER: 1_000_000_000,
    BANK_UPGRADE_LUXURIOUS: 6_000_000_000,
    BANK_UPGRADE_PALATIAL: 60_000_000_000,
  };

  let bankMaxCoins = 0;

  // SAFETY: we assume a co-op always has at least a member
  let firstMemberUsername = Object.keys(profile.members)[0] as Uuid;
  let firstMember = profile.members[firstMemberUsername] as unknown as Member;

  let completedTasks = firstMember.leveling.completed_tasks

  // We search for the best bank upgrade achivement to compute the max bank balance.
  completedTasks.forEach((item) => {
    let balance = maxBalance[item];
    if (balance) {
      bankMaxCoins = Math.max(bankMaxCoins, balance);
    }
  });

  return bankMaxCoins;
}

function stackTransactions(first: LocalOperation, second: LocalOperation): LocalOperation | null {
  // Only stack exact same transaction
  if (first.kind === second.kind
    && second.amount === first.amount
    && first.username === second.username
    && first.sender === second.sender
  ) {

    let outTransaction = first;
    outTransaction.repeatCount = (first.repeatCount ?? 0) + 1;

    return outTransaction;
  }

  return null;
}

function calculateUserDelta(operations: LocalOperation[]): { name: Username, delta: number }[] {
  let lastTransactionLastIndex = operations.findLastIndex((transaction) => Date.now() - transaction.timestamp >= 24 * 3600 * 1000);
  let recentTransactions = operations.slice(lastTransactionLastIndex);

  let usersDelta = new Map<Username, number>();

  for (const recentTransaction of recentTransactions) {
    if (recentTransaction.kind === LocalOperationKind.PlayerPurse
      || recentTransaction.kind === LocalOperationKind.PlayerTransfer) {
      // SAFETY: we just validated the operation kind
      let username = recentTransaction.username!;
      let amount = recentTransaction.amount!;

      const previousAmount = usersDelta.get(username) ?? 0;
      usersDelta.set(username, previousAmount + amount);
    }
  }

  return Array.from(usersDelta.entries()).map(([name, delta]) => ({ name, delta }));
}

async function updateTransactions(profile: Profile) {
  console.log("TSC: Updating");

  let banking = profile.banking as Banking

  let newTransactions: LocalOperation[] = banking.transactions
    .filter((transaction) => transaction.timestamp > (db.lastTransactionTimestamp ?? 0))
    .map(({ action, amount, initiator_name, timestamp }): LocalOperation => {
      let { username, isBankInterest } = processInitiatorName(initiator_name);
      if (isBankInterest) {
        return ({
          kind: LocalOperationKind.BankInterests,
          timestamp,
          amount,
        });
      } else if (username) {
        return ({
          kind: LocalOperationKind.PlayerPurse,
          amount: action === TransactionAction.Withdraw
            ? -amount
            : action === TransactionAction.Deposit
              ? amount : 0,
          timestamp,
          username,
          repeatCount: 1
        });
      } else {
        throw new Error("Invalid transaction recieved from Hypixel API.")
      }
    });

  if (newTransactions.length >= 50) {
    console.warn("TSC WARN: there are 50 new transactions, maybe some were not correctly registered");
    newTransactions.push({
      kind: LocalOperationKind.WeirdWaypoint,
      timestamp: Date.now(),
    })
  }

  let lastTransaction = db.operations.pop() ?? null;

  for (const transaction of newTransactions) {
    console.log(`TSC NEW: ${transaction.username} has ${transaction.kind} ${transaction.amount} ¤`)

    if (transaction.kind === LocalOperationKind.PlayerPurse) {
      // SAFETY: we just checked the local operation kind
      let username = transaction.username!;
      let amount = transaction.amount!;

      if (!db.users[username]) {
        db.users[username] = 0;
      }

      db.users[username] += amount;
    }

    if (lastTransaction) {
      let stackedTransaction = stackTransactions(lastTransaction, transaction);
      if (stackedTransaction) {
        lastTransaction = stackedTransaction;
      } else {
        db.operations.push(lastTransaction)
        lastTransaction = transaction;
      }
    } else {
      lastTransaction = transaction;
    }

    db.lastTransactionTimestamp = transaction.timestamp;
  }

  if (lastTransaction) {
    db.operations.push(lastTransaction)
  }

  let sum = Object.values(db.users).reduce((balance, userBalance) => balance + userBalance, 0) + db.bankInterests;
  let drift = Math.abs(banking.balance - sum);
  if (drift > 1) console.warn(`TSC DRIFT: found ${drift} between balance (${banking.balance}) and sum (${sum})`)
  db.drift = drift;
  db.balance = banking.balance;
  db.maxBalance = getBankLevel(profile);

  flushDatabase();
}

// Styled username may have a single mc color code at the start in the case of ranks
function processInitiatorName(initiatorName: StyledUsername): { username?: Username, isBankInterest?: true } {
  if (initiatorName == "Bank Interest"
    || initiatorName == "Bank Interest (x2)") {
    return { isBankInterest: true };
  }

  if (initiatorName.charAt(0) == "§") {
    // Strip `§a` Minecraft display style tag
    return { username: initiatorName.slice(2) as Username };
  } else {
    return { username: initiatorName as unknown as Username };
  }
}

async function renderHtml() {
  type UserBalance = { name: Username, commonBalance: number };
  type TemplateContext = Pick<DataFile, 'lastTransactionTimestamp' | 'balance' | 'operations' | 'drift' | 'bankInterests' | 'maxBalance'>
    & { users: UserBalance[]; lastCheckTimestamp: number; totalNumberOfOperations: number; usersDelta: { name: Username; delta: number; }[] }
  let template = compile<TemplateContext>(await Bun.file("./templates/index.html.hbs").text());

  let helpers = {
    formatBalance(balance: number): string {
      return new Intl.NumberFormat('fr-FR', {
        maximumFractionDigits: 0,
        style: "currency",
        currency: "XXX",
        currencyDisplay: "symbol",
      }).format(balance);
    },
    formatTimestamp(timestamp: number): string {
      return new Intl.DateTimeFormat('fr-FR', {
        month: "numeric",
        day: "numeric",
        hour: "numeric",
        minute: "numeric",
        timeZone: "Europe/Paris",
      }).format(timestamp);
    },
    computePercentage: (balance: number, maxBalance: number): number => Math.round((balance / maxBalance) * 100),
    absolute: Math.abs,
    isAmountImportant: (amount: number) => (amount >= 5_000_000),
    isAmountNegative: (amount: number) => (amount < 0),
    isDriftImportant: (drift: number): boolean => (drift > 1),
    isWithdrawal: (operation: LocalOperation): boolean => operation.kind === LocalOperationKind.PlayerPurse && operation.amount! < 0,
    isDeposit: (operation: LocalOperation): boolean => operation.kind === LocalOperationKind.PlayerPurse && operation.amount! > 0,
    isPlayerTransfer: (operation: LocalOperation): boolean => (operation.kind === LocalOperationKind.PlayerTransfer),
    isStackedTransaction: (stackSize: number): boolean => (stackSize > 1),
  }

  let usersWithBalance = Object.entries(db.users)
    .map(([name, commonBalance]) => ({ name: name as Username, commonBalance }))
    .sort((a, b) => b.commonBalance - a.commonBalance);

  let html = template({
    balance: db.balance,
    maxBalance: db.maxBalance,
    bankInterests: db.bankInterests,
    drift: db.drift,
    lastCheckTimestamp,
    lastTransactionTimestamp: db.lastTransactionTimestamp,
    operations: db.operations.reverse().slice(0, 25),
    totalNumberOfOperations: db.operations.length,
    usersDelta: calculateUserDelta(db.operations).sort((a, b) => b.delta - a.delta),
    users: usersWithBalance,
  }, { helpers });

  return new Response(html, { headers: { "Content-Type": "text/html" } });
}

async function processTransfer(amount: number, sender: Username, reciever: Username) {
  if (sender === reciever) return;
  
  let newTransfer: LocalOperation = {
    amount,
    sender,
    username: reciever,
    kind: LocalOperationKind.PlayerTransfer,
    timestamp: Date.now(),
  };

  console.log(`TSF NEW: ${newTransfer.sender} has ${newTransfer.kind} ${newTransfer.amount} to ${newTransfer.username} ¤`)

  if (sender === "@bank-interest" || reciever === "@bank-interest") {
    throw new Error("not implemented");
  };

  if (!Object.keys(db.users).includes(sender) || !Object.keys(db.users).includes(reciever)) {
    throw new Error("User does not exist");
  }

  db.users[newTransfer.username!] += amount
  db.users[newTransfer.sender!] -= amount

  db.lastTransactionTimestamp = newTransfer.timestamp;
  db.operations.push(newTransfer);
}

async function runMigrations() {
  let db: DataFile = await Bun.file(DB_FILE).json();

  if (db.version === 0 || db.version === 1) {
    throw new Error("database file version is too old");
  }

  if (db.version === 2) {
    throw new Error("no migration yet");
  }

  if (db.version === DB_VERSION) {
    // the database is up-to-date
    return;
  } else {
    throw new Error("could not migrate db on startup")
  }
}

// Startup
await runMigrations();

// Server
const server = Bun.serve({
  async fetch(request, server) {
    const url = new URL(request.url);
    if (url.pathname === "/") return renderHtml();
    else if (url.pathname === "/ws") { server.upgrade(request); return; }
    else if (url.pathname === "/styles.css") return new Response(Bun.file("./static/styles.css"));
    else if (url.pathname === "/script.js") return new Response(Bun.file("./static/script.js"));
    else if (url.pathname === "/favicon.ico") return new Response(Bun.file("./static/favicon.ico"));
    else return new Response("404!", { status: 404 });
  },

  websocket: {
    open(ws) { ws.subscribe("reload"); },
    close(_ws, _code, _message) { },
    async message(_ws, message: string) {
      let args: string[] = message.split(";")
      if (args[0] === "reload") {
        console.log(`<== WS ACTION: reloading ==>`);
        fetchApi();
      } else if (args[0] === "transfer") {
        console.log(`<== WS ACTION: transfer ==>`);
        let amount = parseInt(args[1]);
        let fromUser = args[2] as Username;
        let toUser = args[3] as Username;

        await processTransfer(amount, fromUser, toUser);
        server.publish("reload", "reload");
      } else {
        console.error(`WS: unknown message ${message}`);
      }
    },
  }
});

// Fetch once on startup and then fetch every 10m
await fetchApi();
setInterval(fetchApi, 10 * 60 * 1000)
