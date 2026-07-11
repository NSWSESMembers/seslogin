import StatusCurrentDisplay from "../components/StatusCurrentDisplay";

const now = Date.now() / 1000;
const h = (hours: number) => now - hours * 3600;

const MOCK_PERIODS = [
  { id: "1", startTime: h(1), name: "Alice Anderson" },
  { id: "2", startTime: h(2), name: "Bob Baker" },
  { id: "3", startTime: h(3), name: "Carol Clark" },
  { id: "4", startTime: h(4), name: "David Davis" },
  { id: "5", startTime: h(5), name: "Eve Evans" },
  { id: "6", startTime: h(6), name: "Frank Foster" },
  { id: "7", startTime: h(7), name: "Grace Green" },
  { id: "8", startTime: h(7.5), name: "Henry Harris" },
  { id: "9", startTime: h(8.5), name: "Isla Irving" },
  { id: "10", startTime: h(9), name: "Jack Jones" },
  { id: "11", startTime: h(9.5), name: "Karen King" },
  { id: "12", startTime: h(10), name: "Liam Lewis" },
  { id: "13", startTime: h(10.5), name: "Mia Martin" },
  { id: "14", startTime: h(11), name: "Nathan Nelson" },
  { id: "15", startTime: h(11.5), name: "Olivia Owen" },
  { id: "16", startTime: h(12.5), name: "Peter Parker" },
  { id: "17", startTime: h(13), name: "Quinn Quinn" },
  { id: "18", startTime: h(13.5), name: "Rachel Roberts" },
  { id: "19", startTime: h(14), name: "Sam Scott" },
  { id: "20", startTime: h(14.5), name: "Tara Taylor" },
  { id: "21", startTime: h(15), name: "Uma Underwood" },
  { id: "22", startTime: h(2.5), name: "Victor Vance" },
  { id: "23", startTime: h(3.5), name: "Wendy Walker" },
  { id: "24", startTime: h(5.5), name: "Xander Xavier" },
  { id: "25", startTime: h(6.5), name: "Yasmine Young" },
  { id: "26", startTime: h(8), name: "Zoe Zhang" },
  { id: "27", startTime: h(9.2), name: "Aaron Abbott" },
  { id: "28", startTime: h(11.8), name: "Bella Brooks" },
  { id: "29", startTime: h(12.2), name: "Carlos Cruz" },
  { id: "30", startTime: h(16), name: "Diana Dixon" },
  { id: "31", startTime: h(2.2), name: "Jamie Guest (Guest)" },
];

export default function StatusDemo() {
  return <StatusCurrentDisplay periods={MOCK_PERIODS} />;
}
