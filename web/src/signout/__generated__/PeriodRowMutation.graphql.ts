/**
 * @generated SignedSource<<fd08a6110ea43b7f776a2554969e65e8>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type PeriodRowMutation$variables = {
  categoryId: string;
  endTime: number;
  periodId: string;
  personId: string;
  startTime: number;
};
export type PeriodRowMutation$data = {
  readonly memberSignOut: {
    readonly id: string;
  };
};
export type PeriodRowMutation = {
  response: PeriodRowMutation$data;
  variables: PeriodRowMutation$variables;
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
  "name": "periodId"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "personId"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "startTime"
},
v5 = [
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
        "name": "periodId",
        "variableName": "periodId"
      },
      {
        "kind": "Variable",
        "name": "personId",
        "variableName": "personId"
      },
      {
        "kind": "Variable",
        "name": "startTime",
        "variableName": "startTime"
      }
    ],
    "concreteType": "Period",
    "kind": "LinkedField",
    "name": "memberSignOut",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/),
      (v3/*: any*/),
      (v4/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "PeriodRowMutation",
    "selections": (v5/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v3/*: any*/),
      (v2/*: any*/),
      (v0/*: any*/),
      (v4/*: any*/),
      (v1/*: any*/)
    ],
    "kind": "Operation",
    "name": "PeriodRowMutation",
    "selections": (v5/*: any*/)
  },
  "params": {
    "cacheID": "fb753fdb4da08709dc743ed6baedbded",
    "id": null,
    "metadata": {},
    "name": "PeriodRowMutation",
    "operationKind": "mutation",
    "text": "mutation PeriodRowMutation(\n  $personId: ID!\n  $periodId: ID!\n  $categoryId: ID!\n  $startTime: Int!\n  $endTime: Int!\n) {\n  memberSignOut(personId: $personId, periodId: $periodId, categoryId: $categoryId, startTime: $startTime, endTime: $endTime) {\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "54f3da9eaf0eeb1164619bbc37dbfeec";

export default node;
