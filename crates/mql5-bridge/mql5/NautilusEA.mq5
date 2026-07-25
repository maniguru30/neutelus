//+------------------------------------------------------------------+
//|                                          NautilusHFT_EA.mq5      |
//|                                      NautilusTrader MQL5 Bridge  |
//+------------------------------------------------------------------+
#property copyright "NautilusTrader"
#property link      "https://nautilustrader.io"
#property version   "1.00"
#property description "HFT EA powered by NautilusTrader Rust Engine"
#property description "Imports nautilus_mql5_bridge.dll for signal generation"

#include <Trade\Trade.mqh>
#include <Trade\PositionInfo.mqh>
#include <Trade\SymbolInfo.mqh>
#include <Trade\AccountInfo.mqh>

//+------------------------------------------------------------------+
//| DLL imports                                                      |
//+------------------------------------------------------------------+
#import "nautilus_mql5_bridge.dll"
int   nt_init(uchar &config_json[]);
void  nt_deinit(int handle);
int   nt_on_imbalance(int handle, uchar &symbol[], double imbalance);
int   nt_on_orderflow(int handle, uchar &symbol[], double delta, double volume);
#import

//+------------------------------------------------------------------+
//| Enums                                                            |
//+------------------------------------------------------------------+
enum ENUM_SIGNAL {
   SIGNAL_NONE    = 0,
   SIGNAL_BUY     = 1,
   SIGNAL_SELL    = 2,
   SIGNAL_EXIT    = 3,
   SIGNAL_ERROR   = -1
};

enum ENUM_STRATEGY_TYPE {
   STRATEGY_ORDERFLOW,            // Order Flow (Cumulative Delta)
   STRATEGY_ORDERBOOK_IMBALANCE   // Order Book Imbalance
};

enum ENUM_RISK_MODE {
   RISK_FIXED_LOT,     // Fixed lot size
   RISK_PERCENT        // % of balance
};

//+------------------------------------------------------------------+
//| Input parameters                                                 |
//+------------------------------------------------------------------+
input group "=== NautilusTrader Bridge ==="
input string InpConfig_       = "";                              // Config JSON (leave empty for defaults)
input ENUM_STRATEGY_TYPE InpStrategy = STRATEGY_ORDERFLOW;       // Strategy type

input group "=== Order Flow (Cumulative Delta) ==="
input int               InpDivergenceLookback = 50;              // Tick window
input double            InpDivergenceThreshold = 5.0;            // Entry threshold (cumulative delta value)

input group "=== Order Book Imbalance ==="
input int               InpSmoothingPeriod = 20;                // Smoothing period
input double            InpEntryThreshold  = 0.3;               // Entry threshold
input double            InpExitThreshold   = 0.1;               // Exit threshold

input group "=== Risk Management ==="
input double            InpLotSize      = 0.1;                  // Fixed lot size
input ENUM_RISK_MODE    InpRiskMode     = RISK_FIXED_LOT;       // Risk mode
input double            InpRiskPercent  = 1.0;                  // Risk % per trade
input double            InpStopLoss     = 20.0;                 // Stop loss (points)
input double            InpTakeProfit   = 40.0;                 // Take profit (points)
input int               InpMaxPositions = 1;                    // Max open positions

input group "=== Filters ==="
input int               InpMaxSpread    = 0;                    // Max spread (points, 0=off)
input bool              InpTradingHours = false;                // Use trading hours
input string            InpStartHour    = "00:00";              // Trading start time
input string            InpEndHour      = "23:59";              // Trading end time

input group "=== Debug ==="
input bool              InpVerboseLog   = false;                // Verbose logging

//+------------------------------------------------------------------+
//| Global variables                                                  |
//+------------------------------------------------------------------+
int           gEngineHandle   = -1;
CTrade        gTrade;
CPositionInfo gPosition;
CSymbolInfo   gSymbol;
CAccountInfo  gAccount;
int           gPositionCount = 0;
bool          gInitialized   = false;

// Order flow state
double        gLastTickPrice = 0;
int           gLastTickDir   = 0;

// Byte array for DLL calls (uchar avoids MT5 x64 UTF-16 string issue)
uchar         gSymbolBytes[];

//+------------------------------------------------------------------+
//| Expert initialization function                                    |
//+------------------------------------------------------------------+
int OnInit() {
   if (!gSymbol.Name(_Symbol)) {
      Print("Failed to get symbol info for ", _Symbol);
      return INIT_FAILED;
   }
   gSymbol.RefreshRates();

   StringToCharArray(_Symbol, gSymbolBytes);

   string config = InpConfig_;
   if (config == "") {
      config = BuildConfigJSON();
   }
   uchar configBytes[];
   StringToCharArray(config, configBytes);

   gEngineHandle = nt_init(configBytes);
   if (gEngineHandle < 0) {
      Print("ERROR: Failed to initialize engine (code: ", gEngineHandle, ")");
      return INIT_FAILED;
   }

   gTrade.SetExpertMagicNumber(999001);
   gTrade.SetDeviationInPoints(5);
   gTrade.SetAsyncMode(false);

   if (InpStrategy == STRATEGY_ORDERBOOK_IMBALANCE) {
      if (!MarketBookAdd(_Symbol)) {
         Print("WARNING: MarketBookAdd failed for ", _Symbol, " (depth unavailable)");
      }
   }

   MqlTick initTick;
   if (SymbolInfoTick(_Symbol, initTick)) {
      gLastTickPrice = initTick.last;
   }

   gInitialized = true;
   Print("NautilusHFT EA initialized on ", _Symbol, " | Handle: ", gEngineHandle);
   return INIT_SUCCEEDED;
}

//+------------------------------------------------------------------+
//| Expert deinitialization function                                  |
//+------------------------------------------------------------------+
void OnDeinit(const int reason) {
   if (gEngineHandle >= 0) {
      nt_deinit(gEngineHandle);
      gEngineHandle = -1;
   }
   MarketBookRelease(_Symbol);
   gInitialized = false;
}

//+------------------------------------------------------------------+
//| Expert tick function                                              |
//+------------------------------------------------------------------+
void OnTick() {
   if (!gInitialized || gEngineHandle < 0) return;

   gSymbol.Refresh();
   gSymbol.RefreshRates();

   MqlTick tick;
   if (!SymbolInfoTick(_Symbol, tick)) return;

   double spread = (tick.ask - tick.bid) / gSymbol.Point();
   if (InpMaxSpread > 0 && spread > InpMaxSpread) return;

   if (InpTradingHours && !IsInTradingHours()) return;

   int signal = SIGNAL_NONE;

   // ── Order Flow (Cumulative Delta) ────────────────────────────
   if (InpStrategy == STRATEGY_ORDERFLOW) {
      double last = tick.last;
      int dir = 0;
      if (last > gLastTickPrice) {
         dir = 1;
      } else if (last < gLastTickPrice) {
         dir = -1;
      } else {
         dir = gLastTickDir;
      }

      double delta = (double)dir;
      signal = nt_on_orderflow(gEngineHandle, gSymbolBytes, delta, last);

      gLastTickPrice = last;
      gLastTickDir = dir;
   }

   // ── Order Book Imbalance ─────────────────────────────────────
   if (InpStrategy == STRATEGY_ORDERBOOK_IMBALANCE) {
      MqlBookInfo book[];
      if (MarketBookGet(_Symbol, book)) {
         double bidVol = 0, askVol = 0;
         for (int i = 0; i < ArraySize(book); i++) {
            if (book[i].type == BOOK_TYPE_BUY) { bidVol += (double)book[i].volume; }
            else if (book[i].type == BOOK_TYPE_SELL) { askVol += (double)book[i].volume; }
         }
         double totalVol = bidVol + askVol;
         if (totalVol > 0) {
            double imbalance = (bidVol - askVol) / totalVol;
            signal = nt_on_imbalance(gEngineHandle, gSymbolBytes, imbalance);
         }
      }
   }

   if (InpVerboseLog && signal != SIGNAL_NONE) {
      Print("Signal: ", SignalToString(signal));
   }

   ProcessSignal(signal, tick);
}

//+------------------------------------------------------------------+
//| Process trading signal                                            |
//+------------------------------------------------------------------+
void ProcessSignal(int signal, MqlTick &tick) {
   gPositionCount = CountPositions();

   switch (signal) {
      case SIGNAL_BUY:
         if (gPositionCount >= InpMaxPositions) return;
         if (!HasPosition(POSITION_TYPE_BUY)) {
            OpenBuy(tick);
         }
         break;

      case SIGNAL_SELL:
         if (gPositionCount >= InpMaxPositions) return;
         if (!HasPosition(POSITION_TYPE_SELL)) {
            OpenSell(tick);
         }
         break;

      case SIGNAL_EXIT:
         CloseAllPositions();
         break;

      case SIGNAL_ERROR:
         if (InpVerboseLog) {
            Print("SIGNAL_ERROR from engine - check DLL");
         }
         break;
   }
}

//+------------------------------------------------------------------+
//| Open buy position                                                 |
//+------------------------------------------------------------------+
void OpenBuy(MqlTick &tick) {
   double lot = CalculateLotSize();
   double sl  = InpStopLoss > 0 ? tick.bid - InpStopLoss * gSymbol.Point() : 0;
   double tp  = InpTakeProfit > 0 ? tick.bid + InpTakeProfit * gSymbol.Point() : 0;

   if (gTrade.Buy(lot, _Symbol, tick.ask, sl, tp, "NautilusHFT")) {
      if (InpVerboseLog) {
         Print("BUY: Lot=", lot, " Price=", tick.ask, " SL=", sl, " TP=", tp);
      }
   } else {
      Print("BUY failed: ", GetLastError());
   }
}

//+------------------------------------------------------------------+
//| Open sell position                                                |
//+------------------------------------------------------------------+
void OpenSell(MqlTick &tick) {
   double lot = CalculateLotSize();
   double sl  = InpStopLoss > 0 ? tick.ask + InpStopLoss * gSymbol.Point() : 0;
   double tp  = InpTakeProfit > 0 ? tick.ask - InpTakeProfit * gSymbol.Point() : 0;

   if (gTrade.Sell(lot, _Symbol, tick.bid, sl, tp, "NautilusHFT")) {
      if (InpVerboseLog) {
         Print("SELL: Lot=", lot, " Price=", tick.bid, " SL=", sl, " TP=", tp);
      }
   } else {
      Print("SELL failed: ", GetLastError());
   }
}

//+------------------------------------------------------------------+
//| Calculate lot size                                                |
//+------------------------------------------------------------------+
double CalculateLotSize() {
   switch (InpRiskMode) {
      case RISK_PERCENT: {
         double balance = gAccount.Balance();
         double riskAmount = balance * InpRiskPercent / 100.0;
         double tickValue = gSymbol.TickValue();
         if (tickValue <= 0.0) {
            return InpLotSize;
         }
         double stopPoints = InpStopLoss > 0 ? InpStopLoss : 20;
         double lot = riskAmount / (stopPoints * tickValue);
         return NormalizeDouble(MathMax(gSymbol.LotsMin(), MathMin(lot, gSymbol.LotsMax())), 2);
      }
      default:
         return NormalizeDouble(InpLotSize, 2);
   }
}

//+------------------------------------------------------------------+
//| Count open positions                                              |
//+------------------------------------------------------------------+
int CountPositions() {
   int count = 0;
   for (int i = PositionsTotal() - 1; i >= 0; i--) {
      if (gPosition.SelectByIndex(i)) {
         if (gPosition.Symbol() == _Symbol && gPosition.Magic() == 999001) {
            count++;
         }
      }
   }
   return count;
}

//+------------------------------------------------------------------+
//| Check if position type exists                                     |
//+------------------------------------------------------------------+
bool HasPosition(ENUM_POSITION_TYPE type) {
   for (int i = PositionsTotal() - 1; i >= 0; i--) {
      if (gPosition.SelectByIndex(i)) {
         if (gPosition.Symbol() == _Symbol && gPosition.Magic() == 999001 && gPosition.PositionType() == type) {
            return true;
         }
      }
   }
   return false;
}

//+------------------------------------------------------------------+
//| Close all positions                                               |
//+------------------------------------------------------------------+
void CloseAllPositions() {
   for (int i = PositionsTotal() - 1; i >= 0; i--) {
      if (gPosition.SelectByIndex(i)) {
         if (gPosition.Symbol() == _Symbol && gPosition.Magic() == 999001) {
            gTrade.PositionClose(_Symbol, 5);
         }
      }
   }
}

//+------------------------------------------------------------------+
//| Check if within trading hours                                     |
//+------------------------------------------------------------------+
bool IsInTradingHours() {
   MqlDateTime dt;
   TimeCurrent(dt);
   int startSeconds = StringToTimeSeconds(InpStartHour);
   int endSeconds = StringToTimeSeconds(InpEndHour);
   int currentSeconds = dt.hour * 3600 + dt.min * 60;

   if (endSeconds >= startSeconds) {
      return currentSeconds >= startSeconds && currentSeconds <= endSeconds;
   } else {
      return currentSeconds >= startSeconds || currentSeconds <= endSeconds;
   }
}

//+------------------------------------------------------------------+
//| Convert time string to seconds                                    |
//+------------------------------------------------------------------+
int StringToTimeSeconds(string timeStr) {
   string parts[];
   if (StringSplit(timeStr, ':', parts) == 2) {
      return (int)parts[0] * 3600 + (int)parts[1] * 60;
   }
   return 0;
}

//+------------------------------------------------------------------+
//| Build config JSON                                                 |
//+------------------------------------------------------------------+
string BuildConfigJSON() {
   string strategyName;
   switch (InpStrategy) {
      case STRATEGY_ORDERBOOK_IMBALANCE:  strategyName = "orderbook_imbalance"; break;
      default:                            strategyName = "orderflow"; break;
   }
   string symbols = "[\"" + _Symbol + "\"]";

   string config = "{";
   config += "\"strategy\":\"" + strategyName + "\",";
   config += "\"symbols\":" + symbols + ",";
   config += "\"smoothing_period\":" + IntegerToString(InpSmoothingPeriod) + ",";
   config += "\"entry_threshold\":" + DoubleToString(InpEntryThreshold, 2) + ",";
   config += "\"exit_threshold\":" + DoubleToString(InpExitThreshold, 2) + ",";
   config += "\"divergence_lookback\":" + IntegerToString(InpDivergenceLookback) + ",";
   config += "\"divergence_threshold\":" + DoubleToString(InpDivergenceThreshold, 1);
   config += "}";

   return config;
}

//+------------------------------------------------------------------+
//| Convert signal to string                                          |
//+------------------------------------------------------------------+
string SignalToString(int signal) {
   switch (signal) {
      case SIGNAL_NONE:  return "NONE";
      case SIGNAL_BUY:   return "BUY";
      case SIGNAL_SELL:  return "SELL";
      case SIGNAL_EXIT:  return "EXIT";
      default: return "UNKNOWN";
   }
}

//+------------------------------------------------------------------+
//| Trade function                                                    |
//+------------------------------------------------------------------+
void OnTrade() {
   gPositionCount = CountPositions();
}

//+------------------------------------------------------------------+
//| Chart event handler                                               |
//+------------------------------------------------------------------+
void OnChartEvent(const int id, const long &lparam, const double &dparam, const string &sparam) {
   if (id == CHARTEVENT_KEYDOWN) {
      if (sparam == "r" || sparam == "R") {
         MarketBookRelease(_Symbol);
         if (gEngineHandle >= 0) {
            nt_deinit(gEngineHandle);
         }
         gLastTickPrice = 0;
         gLastTickDir = 0;
         string config = BuildConfigJSON();
         uchar cfgBytes[];
         StringToCharArray(config, cfgBytes);
         gEngineHandle = nt_init(cfgBytes);
         if (InpStrategy == STRATEGY_ORDERBOOK_IMBALANCE) {
            MarketBookAdd(_Symbol);
         }
         Print("Engine reinitialized");
      }
   }
}
//+------------------------------------------------------------------+
