/**
 * @generated SignedSource<<951ec113ad0536066eb0e0b861643501>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type TrophyChangeKind = "LOST" | "TAKEN" | "%future added value";
export type ScanControllerSignOutWithBadgesMutation$variables = {
  categoryId: string;
  endTime: number;
  id: string;
  startTime: number;
};
export type ScanControllerSignOutWithBadgesMutation$data = {
  readonly scanSignOutWithBadges: {
    readonly awardedBadges: ReadonlyArray<{
      readonly description: string;
      readonly id: string;
      readonly name: string;
      readonly tier: string;
    }>;
    readonly period: {
      readonly category: {
        readonly id: string;
        readonly name: string;
      } | null | undefined;
      readonly endTime: number | null | undefined;
      readonly id: string;
      readonly person: {
        readonly firstName: string;
        readonly id: string;
        readonly lastName: string;
      } | null | undefined;
      readonly startTime: number;
      readonly version: number;
    };
    readonly trophyChanges: ReadonlyArray<{
      readonly kind: TrophyChangeKind;
      readonly locationName: string;
      readonly name: string;
      readonly otherPersonName: string | null | undefined;
      readonly trophyId: string;
      readonly valueLabel: string;
    }>;
  };
};
export type ScanControllerSignOutWithBadgesMutation = {
  response: ScanControllerSignOutWithBadgesMutation$data;
  variables: ScanControllerSignOutWithBadgesMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "categoryId"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "endTime"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "startTime"
},
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v6 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "categoryId",
        "variableName": "categoryId"
      },
      {
        "kind": "Variable",
        "name": "endTime",
        "variableName": "endTime"
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "startTime",
        "variableName": "startTime"
      }
    ],
    "concreteType": "ScanSignOutResult",
    "kind": "LinkedField",
    "name": "scanSignOutWithBadges",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "BadgeAward",
        "kind": "LinkedField",
        "name": "awardedBadges",
        "plural": true,
        "selections": [
          (v4/*:: as any*/),
          (v5/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "description",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "tier",
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "TrophyChange",
        "kind": "LinkedField",
        "name": "trophyChanges",
        "plural": true,
        "selections": [
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "trophyId",
            "storageKey": null
          },
          (v5/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "kind",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "valueLabel",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "otherPersonName",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "locationName",
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "Period",
        "kind": "LinkedField",
        "name": "period",
        "plural": false,
        "selections": [
          (v4/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "version",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "person",
            "plural": false,
            "selections": [
              (v4/*:: as any*/),
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "firstName",
                "storageKey": null
              },
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "lastName",
                "storageKey": null
              }
            ],
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "startTime",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "endTime",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "concreteType": "Category",
            "kind": "LinkedField",
            "name": "category",
            "plural": false,
            "selections": [
              (v4/*:: as any*/),
              (v5/*:: as any*/)
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v3/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ScanControllerSignOutWithBadgesMutation",
    "selections": (v6/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v2/*:: as any*/),
      (v3/*:: as any*/),
      (v1/*:: as any*/),
      (v0/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "ScanControllerSignOutWithBadgesMutation",
    "selections": (v6/*:: as any*/)
  },
  "params": {
    "cacheID": "a6c1f4ff6af5694e773e49648959c4a4",
    "id": null,
    "metadata": {},
    "name": "ScanControllerSignOutWithBadgesMutation",
    "operationKind": "mutation",
    "text": "mutation ScanControllerSignOutWithBadgesMutation(\n  $id: ID!\n  $startTime: Int!\n  $endTime: Int!\n  $categoryId: ID!\n) {\n  scanSignOutWithBadges(id: $id, startTime: $startTime, endTime: $endTime, categoryId: $categoryId) {\n    awardedBadges {\n      id\n      name\n      description\n      tier\n    }\n    trophyChanges {\n      trophyId\n      name\n      kind\n      valueLabel\n      otherPersonName\n      locationName\n    }\n    period {\n      id\n      version\n      person {\n        id\n        firstName\n        lastName\n      }\n      startTime\n      endTime\n      category {\n        id\n        name\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "7106266ffeced82d98d407bc48fc3ce3";

export default node;
