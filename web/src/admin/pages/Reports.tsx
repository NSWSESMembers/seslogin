import { graphql } from "relay-runtime";
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

function csvEscape(value: string): string {
  if (value.includes(",") || value.includes("\n") || value.includes('"')) {
    return `"${value.replaceAll('"', '""')}"`;
  }
  return value;
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

  async function exportReport(format: "csv" | "xlsx") {
    if (!hasValidRange) {
      setErrorText("Start time must be before end time.");
      setSuccessText("");
      return;
    }

    setExportingFormat(format);
    setErrorText("");
    setSuccessText("");

    try {
      const periods: ReportPeriod[] = [];
      let after: string | null = null;
      let hasNextPage = true;

      while (hasNextPage) {
        const data: ReportsQuery["response"] | null | undefined =
          await fetchQuery<ReportsQuery>(
            relayEnvironment,
            graphql`
              query ReportsQuery(
                $location: ID!
                $first: Int!
                $after: String
                $startTime: Int!
                $endTime: Int!
              ) {
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
                        startTime
                        endTime
                        signedInSession {
                          name
                        }
                        signedOutSession {
                          name
                        }
                        category {
                          id
                          name
                        }
                        person {
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
      ];
      const startPart = startInput.replaceAll(":", "-");
      const endPart = endInput.replaceAll(":", "-");

      if (format === "csv") {
        const lines = [header.join(",")];
        for (const period of periods) {
          const startDate = new Date(period.startTime * 1000);
          const endDate = period.endTime
            ? new Date(period.endTime * 1000)
            : null;
          const durationSeconds = period.endTime
            ? period.endTime - period.startTime
            : 0;
          const row = [
            period.id,
            period.person.memberNumber || period.personId,
            `${period.person.firstName} ${period.person.lastName}`.trim(),
            period.category?.name || "",
            startDate.toISOString(),
            period.signedInSession?.name || "",
            endDate ? endDate.toISOString() : "",
            period.signedOutSession?.name || "",
            formatDuration(durationSeconds),
          ];
          lines.push(row.map(csvEscape).join(","));
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
        const rows: Array<Array<string | number | Date>> = periods.map(
          (period) => {
            const startDate = new Date(period.startTime * 1000);
            const endDate = period.endTime
              ? new Date(period.endTime * 1000)
              : new Date(period.startTime * 1000);
            const durationSeconds = period.endTime
              ? period.endTime - period.startTime
              : 0;
            const durationDays = durationSeconds / 86400;

            return [
              period.id,
              period.person.memberNumber || period.personId,
              `${period.person.firstName} ${period.person.lastName}`.trim(),
              period.category?.name || "",
              startDate,
              period.signedInSession?.name || "",
              endDate,
              period.signedOutSession?.name || "",
              durationDays,
            ];
          },
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
    } catch (error) {
      console.error(error);
      setErrorText("Unable to generate report. Please try again.");
      setSuccessText("");
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
