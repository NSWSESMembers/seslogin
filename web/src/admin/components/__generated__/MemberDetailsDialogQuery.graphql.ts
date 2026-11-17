/**
 * @generated SignedSource<<31fded82867918ddb1d51ed18eb1b735>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type MemberDetailsDialogQuery$variables = {
  id: string;
};
export type MemberDetailsDialogQuery$data = {
  readonly person: {
    readonly email: string | null | undefined;
    readonly firstName: string;
    readonly lastName: string;
    readonly mobile: string | null | undefined;
    readonly phones: ReadonlyArray<{
      readonly number: string;
      readonly types: ReadonlyArray<string>;
    }>;
    readonly preferredName: string | null | undefined;
    readonly registrationNumber: string | null | undefined;
    readonly sesApiPersonId: string | null | undefined;
    readonly units: ReadonlyArray<{
      readonly entityCode: string | null | undefined;
      readonly entityId: string;
      readonly entityName: string | null | undefined;
      readonly isPrimary: boolean;
    }>;
  };
};
export type MemberDetailsDialogQuery = {
  response: MemberDetailsDialogQuery$data;
  variables: MemberDetailsDialogQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  }
],
v1 = [
  {
    "kind": "Variable",
    "name": "id",
    "variableName": "id"
  }
],
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "firstName",
  "storageKey": null
},
v3 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "lastName",
  "storageKey": null
},
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "preferredName",
  "storageKey": null
},
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "registrationNumber",
  "storageKey": null
},
v6 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "sesApiPersonId",
  "storageKey": null
},
v7 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "email",
  "storageKey": null
},
v8 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "mobile",
  "storageKey": null
},
v9 = {
  "alias": null,
  "args": null,
  "concreteType": "PersonPhone",
  "kind": "LinkedField",
  "name": "phones",
  "plural": true,
  "selections": [
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "number",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "types",
      "storageKey": null
    }
  ],
  "storageKey": null
},
v10 = {
  "alias": null,
  "args": null,
  "concreteType": "PersonUnitAssignment",
  "kind": "LinkedField",
  "name": "units",
  "plural": true,
  "selections": [
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "entityId",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "entityName",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "entityCode",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "isPrimary",
      "storageKey": null
    }
  ],
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "MemberDetailsDialogQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Person",
        "kind": "LinkedField",
        "name": "person",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          (v3/*:: as any*/),
          (v4/*:: as any*/),
          (v5/*:: as any*/),
          (v6/*:: as any*/),
          (v7/*:: as any*/),
          (v8/*:: as any*/),
          (v9/*:: as any*/),
          (v10/*:: as any*/)
        ],
        "storageKey": null
      }
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "MemberDetailsDialogQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Person",
        "kind": "LinkedField",
        "name": "person",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          (v3/*:: as any*/),
          (v4/*:: as any*/),
          (v5/*:: as any*/),
          (v6/*:: as any*/),
          (v7/*:: as any*/),
          (v8/*:: as any*/),
          (v9/*:: as any*/),
          (v10/*:: as any*/),
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
    ]
  },
  "params": {
    "cacheID": "8212382d09a0d61d85cd28cea47d3850",
    "id": null,
    "metadata": {},
    "name": "MemberDetailsDialogQuery",
    "operationKind": "query",
    "text": "query MemberDetailsDialogQuery(\n  $id: ID!\n) {\n  person(id: $id) {\n    firstName\n    lastName\n    preferredName\n    registrationNumber\n    sesApiPersonId\n    email\n    mobile\n    phones {\n      number\n      types\n    }\n    units {\n      entityId\n      entityName\n      entityCode\n      isPrimary\n    }\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "926ecaf841f91eaa99a86bd05e6ca0d8";

export default node;
