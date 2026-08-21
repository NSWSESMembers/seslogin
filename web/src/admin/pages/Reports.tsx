import { graphql, isValueResult } from "relay-runtime";
import { fetchQuery, useRelayEnvironment } from "react-relay";
import { useSettings } from "../../lib/settings";
import ActivityTimeRange from "../components/ActivityTimeRange";
import useActivityTimeRange from "../components/useActivityTimeRange";
import { useState } from "react";
import type { ReportsQuery } from "./__generated__/ReportsQuery.graphql";
import { Button } from "../../components/ui/Button";

const REPORT_PAGE_SIZE = 1000;
type ReportPeriodEdge = NonNullable<
  NonNullable<
    NonNullable<ReportsQuery["response"]>["location"]
  >["periods"]["edges"][number]
>;
type ReportPeriod = ReportPeriodEdge["node"];
type ReportPeriodsConnection = NonNullable<
  NonNullable<ReportsQuery["response"]>["location"]
>["periods"];

type ResolvedRow = {
  id: string;
  memberId: string;
  name: string;
  categoryName: string;
  startDate: Date;
  endDate: Date | null;
  durationSeconds: number;
  signedInSession: string;
  signedOutSession: string;
  comment: string;
};

const MAX_INCOMPLETE_MESSAGES = 5;

// category/person are @catch'd (see the query below) so a dangling reference
// on one period degrades just that period's row instead of hiding the whole
// report. A failed person lookup must never render as "GUEST" — that would
// mislabel a real member — so it's left blank and the row is flagged
// incomplete instead.
function resolveRow(
  period: ReportPeriod,
  incompleteMessages: string[],
  noteIncomplete: () => void,
): ResolvedRow {
  const categoryOk = isValueResult(period.category);
  const personOk = isValueResult(period.person);
  const personValue = personOk ? period.person.value : null;

  if (!categoryOk || !personOk) {
    noteIncomplete();
    if (incompleteMessages.length < MAX_INCOMPLETE_MESSAGES) {
      const missing = [
        !categoryOk ? "category" : null,
        !personOk ? "person" : null,
      ]
        .filter((field): field is string => field !== null)
        .join(" and ");
      incompleteMessages.push(
        `Period ${period.id}: ${missing} could not be loaded`,
      );
    }
  }

  return {
    id: period.id,
    memberId: !personOk
      ? ""
      : personValue
        ? personValue.memberNumber || period.personId || ""
        : "GUEST",
    name: !personOk
      ? ""
      : personValue
        ? `${personValue.firstName} ${personValue.lastName}`.trim()
        : (period.guestName ?? ""),
    categoryName: categoryOk ? (period.category.value?.name ?? "") : "",
    startDate: new Date(period.startTime * 1000),
    endDate: period.endTime ? new Date(period.endTime * 1000) : null,
    durationSeconds: period.endTime ? period.endTime - period.startTime : 0,
    signedInSession: period.signedInSession?.name || "",
    signedOutSession: period.signedOutSession?.name || "",
    comment: period.comment ?? "",
  };
}

function csvEscape(value: string): string {
  // Guard against CSV formula injection: a cell starting with =, +, -, or @ can be
  // interpreted as a formula by spreadsheet apps. Free-text guest names/reasons now
  // enter the CSV, so prefix such cells with a single quote before quoting.
  let cell = value;
  if (/^[=+\-@]/.test(cell)) {
    cell = `'${cell}`;
  }
  if (cell.includes(",") || cell.includes("\n") || cell.includes('"')) {
    return `"${cell.replaceAll('"', '""')}"`;
  }
  return cell;
}

function formatDuration(seconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(totalSeconds / 3600)
    .toString()
    .padStart(2, "0");
  const minutes = Math.floor((totalSeconds % 3600) / 60)
    .toString()
    .padStart(2, "0");
  const secs = (totalSeconds % 60).toString().padStart(2, "0");
  return `${hours}:${minutes}:${secs}`;
}

export default function Reports() {
  const settings = useSettings();
  const relayEnvironment = useRelayEnvironment();
  const {
    startInput,
    endInput,
    setStartInput,
    setEndInput,
    hasValidRange,
    queryStartTime,
    queryEndTime,
  } = useActivityTimeRange();
  const [exportingFormat, setExportingFormat] = useState<"csv" | "xlsx" | null>(
    null,
  );
  const [errorText, setErrorText] = useState("");
  const [successText, setSuccessText] = useState("");
  const [warningText, setWarningText] = useState("");

  async function exportReport(format: "csv" | "xlsx") {
    if (!hasValidRange) {
      setErrorText("Start time must be before end time.");
      setSuccessText("");
      return;
    }

    setExportingFormat(format);
    setErrorText("");
    setSuccessText("");
    setWarningText("");

    try {
      const periods: ReportPeriod[] = [];
      let after: string | null = null;
      let hasNextPage = true;
      let hadFieldErrors = false;

      while (hasNextPage) {
        let data: ReportsQuery["response"] | null | undefined;
        try {
          data = await fetchQuery<ReportsQuery>(
            relayEnvironment,
            graphql`
              query ReportsQuery(
                $location: ID!
                $first: Int!
                $after: String
                $startTime: Int!
                $endTime: Int!
              ) @throwOnFieldError {
                location(id: $location) {
                  id
                  periods(
                    first: $first
                    after: $after
                    startTime: $startTime
                    endTime: $endTime
                  ) {
                    edges {
                      node {
                        id
                        personId
                        guestName
                        comment
                        startTime
                        endTime
                        signedInSession {
                          name
                        }
                        signedOutSession {
                          name
                        }
                        # @catch so a dangling category/person reference on one
                        # period degrades just that row in the export (marked
                        # incomplete) instead of either hiding the whole
                        # report or (via @throwOnFieldError on the enclosing
                        # query) silently writing a blank/miscategorized cell.
                        category @catch {
                          id
                          name
                        }
                        person @catch {
                          id
                          memberNumber
                          firstName
                          lastName
                        }
                      }
                    }
                    pageInfo {
                      hasNextPage
                      endCursor
                    }
                  }
                }
              }
            `,
            {
              location: settings?.locationId || "",
              first: REPORT_PAGE_SIZE,
              after,
              startTime: queryStartTime,
              endTime: queryEndTime,
            },
          ).toPromise();
        } catch (err) {
          // A field error on something other than category/person (e.g. a
          // corrupt startTime) isn't scoped by @catch, so @throwOnFieldError
          // rejects the whole page instead of resolving it with a Result we
          // could unwrap — that's literally what the query's `errors` key
          // reported, just surfaced to us as a thrown error instead of the
          // raw array. We can't tell which row(s) in the page it was or
          // recover just those, so stop paging here and keep whatever pages
          // already succeeded, and warn rather than failing the whole export.
          console.error("A page of this report failed to load:", err);
          hadFieldErrors = true;
          break;
        }

        const location = data?.location;
        if (!location) {
          break;
        }

        const page: ReportPeriodsConnection = location.periods;
        if (!page) {
          break;
        }

        periods.push(...page.edges.map((edge: ReportPeriodEdge) => edge.node));
        hasNextPage = page.pageInfo.hasNextPage;
        after = page.pageInfo.endCursor ?? null;

        if (hasNextPage && !after) {
          break;
        }
      }

      const header = [
        "ID (period_id)",
        "Member ID",
        "Name",
        "Category Name",
        "Start Time",
        "Sign-In Kiosk",
        "End Time",
        "Sign-Out Kiosk",
        "Duration",
        "Comment",
      ];
      const startPart = startInput.replaceAll(":", "-");
      const endPart = endInput.replaceAll(":", "-");

      let incompleteCount = 0;
      const incompleteMessages: string[] = [];
      const resolvedRows = periods.map((period) =>
        resolveRow(period, incompleteMessages, () => incompleteCount++),
      );

      if (format === "csv") {
        const lines = [header.join(",")];
        for (const row of resolvedRows) {
          const csvRow = [
            row.id,
            row.memberId,
            row.name,
            row.categoryName,
            row.startDate.toISOString(),
            row.signedInSession,
            row.endDate ? row.endDate.toISOString() : "",
            row.signedOutSession,
            formatDuration(row.durationSeconds),
            row.comment,
          ];
          lines.push(csvRow.map(csvEscape).join(","));
        }

        const csvContent = lines.join("\n");
        const blob = new Blob([csvContent], {
          type: "text/csv;charset=utf-8;",
        });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");

        link.href = url;
        link.download = `activity-report-${startPart}-to-${endPart}.csv`;
        document.body.appendChild(link);
        link.click();
        link.remove();
        URL.revokeObjectURL(url);
      } else {
        const rows: Array<Array<string | number | Date>> = resolvedRows.map(
          (row) => [
            row.id,
            row.memberId,
            row.name,
            row.categoryName,
            row.startDate,
            row.signedInSession,
            row.endDate ?? row.startDate,
            row.signedOutSession,
            row.durationSeconds / 86400,
            row.comment,
          ],
        );

        const { default: ExcelJS } = await import("exceljs");
        const workbook = new ExcelJS.Workbook();
        const worksheet = workbook.addWorksheet("Report");

        worksheet.addRow(header);
        for (const row of rows) {
          worksheet.addRow(row);
        }

        for (let rowIndex = 2; rowIndex <= rows.length + 1; rowIndex++) {
          worksheet.getCell(rowIndex, 5).numFmt = "yyyy-mm-dd hh:mm:ss";
          worksheet.getCell(rowIndex, 7).numFmt = "yyyy-mm-dd hh:mm:ss";
          worksheet.getCell(rowIndex, 9).numFmt = "[h]:mm:ss";
        }

        const buffer = await workbook.xlsx.writeBuffer();
        const blob = new Blob([buffer], {
          type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");

        link.href = url;
        link.download = `activity-report-${startPart}-to-${endPart}.xlsx`;
        document.body.appendChild(link);
        link.click();
        link.remove();
        URL.revokeObjectURL(url);
      }

      setSuccessText(
        `Exported ${periods.length} row${periods.length === 1 ? "" : "s"} as ${format.toUpperCase()}.`,
      );

      const warnings: string[] = [];
      if (incompleteCount > 0) {
        const shown =
          incompleteMessages.length < incompleteCount
            ? ` First ${incompleteMessages.length}:`
            : "";
        warnings.push(
          `${incompleteCount} record${incompleteCount === 1 ? "" : "s"} ` +
            `written incomplete — a category or person reference could not ` +
            `be loaded.${shown}\n${incompleteMessages.join("\n")}`,
        );
      }
      if (hadFieldErrors) {
        warnings.push(
          "At least one part of this report hit a server error and had to " +
            "be skipped, so some/all data may be missing from the report. " +
            "Please contact a maintainer for help or try again later.",
        );
      }
      if (warnings.length > 0) {
        setWarningText(`Warning: ${warnings.join("\n\n")}`);
      }
    } catch (error) {
      console.error(error);
      setErrorText("Unable to generate report. Please try again.");
      setSuccessText("");
      setWarningText("");
    } finally {
      setExportingFormat(null);
    }
  }

  function onSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    void exportReport("csv");
  }

  return (
    <>
      <form onSubmit={onSubmit}>
        <ActivityTimeRange
          startInput={startInput}
          endInput={endInput}
          onStartChange={setStartInput}
          onEndChange={setEndInput}
        />
        {!hasValidRange && (
          <p className="font-bold text-red-600">
            Start time must be before end time.
          </p>
        )}
        {errorText && <p className="font-bold text-red-600">{errorText}</p>}
        {successText && (
          <p className="font-bold text-green-700">{successText}</p>
        )}
        {warningText && (
          <p className="font-bold whitespace-pre-line text-orange-600">
            {warningText}
          </p>
        )}
        <div className="flex justify-center gap-2.5 max-md:flex-col max-md:items-center">
          <Button
            type="submit"
            disabled={exportingFormat !== null || !hasValidRange}
          >
            {exportingFormat === "csv" ? "Generating..." : "Download CSV"}
          </Button>
          <Button
            type="button"
            disabled={exportingFormat !== null || !hasValidRange}
            onClick={() => void exportReport("xlsx")}
          >
            {exportingFormat === "xlsx" ? "Generating..." : "Download XLSX"}
          </Button>
        </div>
      </form>
    </>
  );
}
