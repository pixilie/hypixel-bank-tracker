import { compile } from "handlebars";
import "dotenv/config";

const VERSION = 2;

const DB_FILE = "data.json";

// Used to indicate maybe missing transactions, and date of drift finding
const WEIRD_WAYPOINT_USERNAME: Username = "Weird Waypoint" as Username;

// Remarquable username to aggregate bank interests
const BANK_INTEREST_USERNAME: Username = "Bank Interest" as Username;

interface DataFile {
  version: number,
  lastTransactionTimestamp: number;
  balance: number;
  maxBalance: number;
  drift: number;
  users: Record<Username, number>;
  transactions: LocalTransaction[];
}

interface LocalTransaction {
  amount: number;
  timestamp: number;
  username: Username;
  sender: Username | null;
  action: TransactionAction;
  number: number;
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
  /* TO FINISH */
}

interface Leveling {
  completed_tasks: Array<string>;
  /* TO FINISH */
}

interface CommunityUpgrades { /* TODO */ }

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
  Transfer = "TRANSFER"
}

let lastCheckTimestamp: number = 0;

async function fetchApi() {
  console.log(`Fetching fresh information for profile '${process.env.PROFILE_UUID!}'`);

  let url = new URL("https://api.hypixel.net/v2/skyblock/profile");
  url.searchParams.append("key", process.env.API_KEY!);
  url.searchParams.append("profile", process.env.PROFILE_UUID!);

  let data: ProfileResponse = await fetch(url).then((res) => res.json());
  if (!data.success) throw new Error(`There was an error while fetching Hypixel's API: ${data.cause}`);

  updateTransactions(data.profile);
  lastCheckTimestamp = Date.now();

  console.log(`Got fresh information for profile '${process.env.PROFILE_UUID!}', reloading clients`);

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
  let UUID = "050ce16aada3482b8d82e3ee42c6d71b" as Uuid
  let completedTasks = profile.members[UUID].leveling.completed_tasks

  completedTasks.forEach((item) => {
    let balance = maxBalance[item];
    if (balance) {
      bankMaxCoins = Math.max(bankMaxCoins, balance);
    }
  });

  return bankMaxCoins;
}

function stackTransactions(first: LocalTransaction, second: LocalTransaction): LocalTransaction | null {
  // Only stack exact same transaction
  if (first.sender === second.sender
    && first.username === second.username
    && first.action === second.action
    && second.amount === first.amount) {

    let outTransaction = first;
    outTransaction.number += 1;

    return outTransaction;
  }

  return null;
}

function calculateUserDelta(db: DataFile): { name: Username, delta: number }[] {
  let transactions = Object.entries(db.transactions)
    .map(([_, localTransaction]) => ({ user: localTransaction.username, amount: localTransaction.amount, timestamp: localTransaction.timestamp }))
    .reverse();

  let lastTransactionIndex = transactions.findIndex((transaction) => Date.now() - transaction.timestamp >= 24 * 3600 * 1000);
  let recentTransactions = transactions.slice(0, lastTransactionIndex);

  let usersDelta = new Map<Username, number>();

  for (const recentTransaction of recentTransactions) {
    const previousAmount = usersDelta.get(recentTransaction.user) ?? 0;
    usersDelta.set(recentTransaction.user, previousAmount + recentTransaction.amount);
  }

  return Array.from(usersDelta.entries()).map(([name, delta]) => ({ name, delta }));
}

async function updateTransactions(profile: Profile) {
  console.log("TSC: Updating");

  const db: DataFile = await Bun.file(DB_FILE).json();

  let banking = profile.banking as Banking

  let newTransactions: LocalTransaction[] = banking.transactions
    .filter((transaction) => transaction.timestamp > (db.lastTransactionTimestamp ?? 0))
    .map(({ action, amount, initiator_name, timestamp }): LocalTransaction => ({
      action,
      amount,
      timestamp,
      username: processDisplayUsername(initiator_name),
      sender: null,
      number: 1
    }));


  if (newTransactions.length >= 50) {
    console.warn("TSC WARN: there are 50 new transactions, maybe some were not correctly registered");
    newTransactions.push({
      action: TransactionAction.Deposit,
      amount: 0,
      timestamp: Date.now(),
      username: WEIRD_WAYPOINT_USERNAME,
      sender: null,
      number: 1
    })
  }

  let lastTransaction = db.transactions.pop() ?? null;

  for (const transaction of newTransactions) {
    console.log(`TSC NEW: ${transaction.username} has ${transaction.action} ${transaction.amount} ¤`)

    if (!db.users[transaction.username]) {
      db.users[transaction.username] = 0;
    }

    switch (transaction.action) {
      case TransactionAction.Deposit:
        db.users[transaction.username] += transaction.amount;
        break;
      case TransactionAction.Withdraw:
        db.users[transaction.username] -= transaction.amount;
        break;
      case TransactionAction.Transfer:
        console.error("TSF: Cannot get transfer as a TransactionAction from hypixel")
        break;
    }

    if (lastTransaction) {
      let stackedTransaction = stackTransactions(lastTransaction, transaction);
      if (stackedTransaction) {
        lastTransaction = stackedTransaction;
      } else {
        db.transactions.push(lastTransaction)
        lastTransaction = transaction;
      }

    } else {
      lastTransaction = transaction;
    }

    db.lastTransactionTimestamp = transaction.timestamp;
  }

  if (lastTransaction) {
    db.transactions.push(lastTransaction)
  }

  let sum = Object.values(db.users).reduce((sum, a) => sum + a, 0);
  let drift = Math.abs(banking.balance - sum);
  if (drift > 1) console.warn(`TSC DRIFT: found ${drift} between balance (${banking.balance}) and sum (${sum})`)
  db.drift = drift;
  db.balance = banking.balance;
  db.maxBalance = getBankLevel(profile);

  await Bun.write(DB_FILE, JSON.stringify(db));
}

// Styled username may have a single mc color code at the start in the case of ranks
function processDisplayUsername(display: StyledUsername): Username {
  // Avoid duplicate Bank entity
  if (display == "Bank Interest (x2)") return BANK_INTEREST_USERNAME;

  if (display.charAt(0) == "§") {
    // Strip `§a` Minecraft display style tag
    return display.slice(2) as Username
  } else {
    return display as unknown as Username;
  }
}

async function renderHtml() {
  const db: DataFile = await Bun.file(DB_FILE).json();

  type UserBalance = { name: Username, commonBalance: number, personalBalance: number };
  type TemplateContext = Pick<DataFile, 'lastTransactionTimestamp' | 'balance' | 'transactions' | 'drift'>
    & { users: UserBalance[]; lastCheckTimestamp: number; totalNumberOfTransactions: number; bankInterest: number | undefined, allUsers: UserBalance[], usersDelta: { name: Username; delta: number; }[] }
  let template = compile<TemplateContext>(await Bun.file("index.html.hbs").text());

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
    calculPercentage(balance: number, maxBalance: number): number {
      return Math.round((balance / maxBalance) * 100)
    },
    isDeposit: (action: string): boolean => (action === TransactionAction.Deposit),
    isAmountImportant: (amount: number) => (amount >= 5_000_000),
    isAmountNegative: (amount: number) => (amount < 0),
    isDriftImportant: (drift: number): boolean => (drift > 1),
    isTransfer: (action: string): boolean => (action === TransactionAction.Transfer),
    isStackedTransaction: (stackSize: number): boolean => (stackSize >= 2),
  }

  let html = template({
    ...db,
    users: Object.entries(db.users)
      .map(([name, commonBalance]) => ({ name: name as Username, commonBalance, personalBalance: 0 }))
      .sort((a, b) => b.commonBalance - a.commonBalance)
      .filter((user) => user.name !== BANK_INTEREST_USERNAME),
    allUsers: Object.entries(db.users)
      .map(([name, commonBalance]) => ({ name: name as Username, commonBalance, personalBalance: 0 }))
      .sort((a, b) => b.commonBalance - a.commonBalance),
    bankInterest: Object.entries(db.users)
      .map(([name, commonBalance]) => ({ name: name as Username, commonBalance, personalBalance: 0 }))
      .sort((a, b) => b.commonBalance - a.commonBalance)
      .find((user) => user.name === BANK_INTEREST_USERNAME)
      ?.commonBalance,
    usersDelta: calculateUserDelta(db)
      .sort((a, b) => b.delta - a.delta)
      .filter((user) => user.name != BANK_INTEREST_USERNAME),
    transactions: db.transactions.reverse().slice(0, 25),
    totalNumberOfTransactions: db.transactions.length,
    lastCheckTimestamp
  }, { helpers });
  return new Response(html, { headers: { "Content-Type": "text/html" } });
}

async function processTransfer(amount: number, sender: Username, reciever: Username) {
  let newTransfer: LocalTransaction = {
    amount,
    sender,
    username: reciever,
    action: TransactionAction.Transfer,
    timestamp: Date.now(),
    number: 1
  };

  console.log(`TSF NEW: ${newTransfer.sender} has ${newTransfer.action} ${newTransfer.amount} to ${newTransfer.username} ¤`)

  const db: DataFile = await Bun.file(DB_FILE).json();

  // Initalize users if not already done
  if (!db.users[newTransfer.sender!]) db.users[newTransfer.sender!] = 0;
  if (!db.users[newTransfer.username]) db.users[newTransfer.username] = 0;

  db.users[newTransfer.sender!] -= amount
  db.users[newTransfer.username] += amount
  db.lastTransactionTimestamp = newTransfer.timestamp;
  db.transactions = db.transactions.concat([newTransfer]);

  await Bun.write(DB_FILE, JSON.stringify(db));
}

function normalizeTransactions(transactions: LocalTransaction[]): LocalTransaction[] {
  const newFormat: LocalTransaction[] = [];

  for (const transaction of transactions) {
    const lastTransaction = newFormat[newFormat.length - 1];

    if (
      lastTransaction &&
      lastTransaction.username === transaction.username &&
      lastTransaction.action === transaction.action &&
      lastTransaction.amount === transaction.amount &&
      lastTransaction.sender === transaction.sender
    ) {
      lastTransaction.number = (lastTransaction.number ?? 1) + 1;
    } else {
      newFormat.push({ ...transaction, number: transaction.number ?? 1 });
    }
  }

  return newFormat;
}

async function runMigrations() {
  let db: DataFile = await Bun.file(DB_FILE).json();
  if (db.version === 0 || db.version === 1) {
    db.transactions = normalizeTransactions(db.transactions);
    db.version = 2;
    await Bun.write(DB_FILE, JSON.stringify(db));
  }

  if (db.version === VERSION) {
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
    else if (url.pathname === "/styles.css") return new Response(Bun.file("styles.css"));
    else if (url.pathname === "/script.js") return new Response(Bun.file("script.js"));
    else if (url.pathname === "/favicon.ico") return new Response(Bun.file("favicon.ico"));
    else if (url.pathname === "/ws") { server.upgrade(request); return; }

    return new Response("404!", { status: 404 });
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
