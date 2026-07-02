/**
 * @generated SignedSource<<8ed4a4d53dece19c1e29007bbd5cb981>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type LocationEditMutation$variables = {
  badgeWeeklyDigestEnabled?: boolean | null | undefined;
  enabled: boolean;
  gamificationEnabled?: boolean | null | undefined;
  id: string;
  name: string;
  nitcEnabled?: number | null | undefined;
};
export type LocationEditMutation$data = {
  readonly updateLocation: {
    readonly badgeWeeklyDigestEnabled: boolean;
    readonly enabled: boolean;
    readonly gamificationEnabled: boolean;
    readonly id: string;
    readonly name: string;
    readonly nitcEnabled: number | null | undefined;
  };
};
export type LocationEditMutation = {
  response: LocationEditMutation$data;
  variables: LocationEditMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "badgeWeeklyDigestEnabled"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "enabled"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "gamificationEnabled"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "name"
},
v5 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcEnabled"
},
v6 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "badgeWeeklyDigestEnabled",
        "variableName": "badgeWeeklyDigestEnabled"
      },
      {
        "kind": "Variable",
        "name": "enabled",
        "variableName": "enabled"
      },
      {
        "kind": "Variable",
        "name": "gamificationEnabled",
        "variableName": "gamificationEnabled"
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      },
      {
        "kind": "Variable",
        "name": "nitcEnabled",
        "variableName": "nitcEnabled"
      }
    ],
    "concreteType": "Location",
    "kind": "LinkedField",
    "name": "updateLocation",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "name",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "enabled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcEnabled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "gamificationEnabled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "badgeWeeklyDigestEnabled",
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
      (v4/*: any*/),
      (v5/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "LocationEditMutation",
    "selections": (v6/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v3/*: any*/),
      (v4/*: any*/),
      (v1/*: any*/),
      (v5/*: any*/),
      (v2/*: any*/),
      (v0/*: any*/)
    ],
    "kind": "Operation",
    "name": "LocationEditMutation",
    "selections": (v6/*: any*/)
  },
  "params": {
    "cacheID": "c6e6b638797465c1e8b1d89b319d49c6",
    "id": null,
    "metadata": {},
    "name": "LocationEditMutation",
    "operationKind": "mutation",
    "text": "mutation LocationEditMutation(\n  $id: ID!\n  $name: String!\n  $enabled: Boolean!\n  $nitcEnabled: Int\n  $gamificationEnabled: Boolean\n  $badgeWeeklyDigestEnabled: Boolean\n) {\n  updateLocation(id: $id, name: $name, enabled: $enabled, nitcEnabled: $nitcEnabled, gamificationEnabled: $gamificationEnabled, badgeWeeklyDigestEnabled: $badgeWeeklyDigestEnabled) {\n    id\n    name\n    enabled\n    nitcEnabled\n    gamificationEnabled\n    badgeWeeklyDigestEnabled\n  }\n}\n"
  }
};
})();

(node as any).hash = "4a069fb9c795f94c99c6d1428376d46e";

export default node;
